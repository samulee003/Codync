import CodyncKit
import SwiftUI

/// A game-style stamina bar under a bot's avatar: how much of its agent's plan is left (the tightest limit), green while
/// plenty, amber under half, red when nearly out.
struct StaminaBar: View {
    let stamina: Stamina
    let width: CGFloat

    private var color: Color {
        switch stamina.level {
        case .full: Palette.added
        case .low: Palette.warning
        case .out: Palette.danger
        }
    }

    var body: some View {
        Capsule()
            .fill(Palette.text.opacity(0.12))
            .frame(width: width, height: 4)
            .overlay(alignment: .leading) {
                Capsule().fill(color).frame(width: width * CGFloat(stamina.left) / 100)
            }
            .animation(Motion.layout, value: stamina.left)
            .accessibilityElement()
            .accessibilityLabel(L10n.tr(stamina.label))
            .accessibilityValue(L10n.tr("{percent}% left").replacingOccurrences(of: "{percent}", with: "\(stamina.left)"))
    }
}
