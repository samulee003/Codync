//! Speech-to-text (`transcribe`) and text-to-speech (`speak`) on the user's provider keys.

use super::{Provider, TIMEOUT, checked, key, send};
use crate::store::Store;
use anyhow::{Result, anyhow, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

/// Speech to text: keep the speaker's language and script, and coding terms as written.
const TRANSCRIBE_HINT: &str = "Keep the speaker's language and script. Keep code, file and product names as written.";

/// Names a speaker is likely to say that speech-to-text would otherwise misspell: the app and
/// the bots on this computer.
fn spoken_names(store: &Store) -> Vec<String> {
    let mut names = vec!["Codync".to_owned()];
    for bot in store.bots().unwrap_or_default() {
        if !bot.deleted && !names.contains(&bot.config.name) {
            names.push(bot.config.name);
        }
    }
    names.truncate(50);
    names
}

fn names_hint(names: &[String]) -> String {
    format!("Names that may come up: {}.", names.join(", "))
}

/// Speech to text for one utterance (`audio`: base64 WAV, 16 kHz mono).
pub async fn transcribe(store: &Store, p: Provider, model: &str, audio: &str) -> Result<Value> {
    let key = key(store, p)?;
    let model = checked(model)?;
    let wav = STANDARD.decode(audio).map_err(|_| anyhow!("audio must be base64"))?;
    let names = spoken_names(store);
    let text = match p {
        Provider::OpenAi => {
            let boundary = format!("codync-{}", uuid::Uuid::new_v4().simple());
            let mut body = Vec::with_capacity(wav.len() + 512);
            body.extend_from_slice(
                format!("--{boundary}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n{model}\r\n").as_bytes(),
            );
            // Keeps Traditional Chinese from coming back Simplified, and code terms intact.
            let mut field = |name: &str, value: &str| {
                body.extend_from_slice(
                    format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                        .as_bytes(),
                );
            };
            field("prompt", &format!("{TRANSCRIBE_HINT} {}", names_hint(&names)));
            // Only gpt-transcribe takes keywords; the older models reject the field.
            if model.starts_with("gpt-transcribe") {
                for name in &names {
                    field("keywords[]", name);
                }
            }
            body.extend_from_slice(
                format!(
                    "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"speech.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
                )
                .as_bytes(),
            );
            body.extend_from_slice(&wav);
            body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
            let req = crate::http()
                .post("https://api.openai.com/v1/audio/transcriptions")
                .bearer_auth(&key)
                .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
                .body(body);
            send(p, req).await?["text"].as_str().unwrap_or_default().to_owned()
        }
        Provider::Gemini => {
            let body = json!({"contents": [{"parts": [
                {"text": format!("Transcribe this audio exactly as spoken. {TRANSCRIBE_HINT} {} Reply with the transcript only; reply with nothing if there is no speech.", names_hint(&names))},
                {"inlineData": {"mimeType": "audio/wav", "data": audio}},
            ]}]});
            let v = send(p, gemini_generate(&key, model).json(&body)).await?;
            v["candidates"][0]["content"]["parts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|part| part["text"].as_str())
                .collect::<String>()
        }
    };
    Ok(json!({"text": text.trim()}))
}

/// Text to speech for one stretch of a reply: `{audio: base64, format: "aac" | "wav"}`.
pub async fn speak(store: &Store, p: Provider, model: &str, voice: &str, text: &str) -> Result<Value> {
    let key = key(store, p)?;
    let (model, voice) = (checked(model)?, checked(voice)?);
    if text.trim().is_empty() || text.len() > 4000 {
        bail!("text must be 1–4000 bytes");
    }
    match p {
        Provider::OpenAi => {
            let body = json!({"model": model, "voice": voice, "input": text, "response_format": "aac"});
            let res = crate::http()
                .post("https://api.openai.com/v1/audio/speech")
                .bearer_auth(&key)
                .json(&body)
                .timeout(TIMEOUT * 4)
                .send()
                .await
                .map_err(|_| anyhow!("Can't reach {}.", p.name()))?;
            let status = res.status();
            let bytes = res.bytes().await.map_err(|_| anyhow!("{} sent no audio.", p.name()))?;
            if !status.is_success() {
                let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                bail!(
                    v["error"]["message"]
                        .as_str()
                        .map_or_else(|| format!("{} answered {status}.", p.name()), str::to_owned)
                );
            }
            Ok(json!({"audio": STANDARD.encode(&bytes), "format": "aac"}))
        }
        Provider::Gemini => {
            let body = json!({
                "contents": [{"parts": [{"text": text}]}],
                "generationConfig": {
                    "responseModalities": ["AUDIO"],
                    "speechConfig": {"voiceConfig": {"prebuiltVoiceConfig": {"voiceName": voice}}},
                },
            });
            let v = send(p, gemini_generate(&key, model).json(&body)).await?;
            let pcm = v["candidates"][0]["content"]["parts"]
                .as_array()
                .into_iter()
                .flatten()
                .find_map(|part| part["inlineData"]["data"].as_str())
                .ok_or_else(|| anyhow!("{} sent no audio.", p.name()))?;
            let pcm = STANDARD.decode(pcm).map_err(|_| anyhow!("{} sent unreadable audio.", p.name()))?;
            Ok(json!({"audio": STANDARD.encode(wav(&pcm, 24_000)), "format": "wav"}))
        }
    }
}

fn gemini_generate(key: &str, model: &str) -> reqwest::RequestBuilder {
    crate::http()
        .post(format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"))
        .header("x-goog-api-key", key)
        .timeout(TIMEOUT * 4)
}

/// 16-bit mono PCM wrapped in a WAV header (Gemini speech is raw PCM).
pub(super) fn wav(pcm: &[u8], rate: u32) -> Vec<u8> {
    let len = u32::try_from(pcm.len()).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(pcm.len() + 44);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(pcm);
    out
}
