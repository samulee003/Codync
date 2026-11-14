import CodyncKit
import SwiftUI

/// A task's work order in its card: the acceptance criteria, each with the latest check's verdict; the plan, and while it
/// waits for the user, approving it or sending it back with what to change; the questions the claimer asks, answered here.
struct TaskWorkOrder: View {
    let task: BoardTask
    let failed: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if !task.criteria.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text(L10n.tr("Acceptance criteria")).font(.caption.weight(.semibold)).foregroundStyle(Palette.secondary)
                    ForEach(task.criteria) { CriterionRow(item: $0) }
                }
            }
            if let plan = task.plan {
                VStack(alignment: .leading, spacing: 4) {
                    Text(L10n.tr(task.approval == .approved ? "Approved plan" : "Proposed plan"))
                        .font(.caption.weight(.semibold)).foregroundStyle(Palette.secondary)
                    Text(plan).font(.callout).textSelection(.enabled)
                }
            }
            if task.awaitsApproval { PlanDecision(task: task, failed: failed) }
            if !task.questions.isEmpty { QuestionList(task: task, failed: failed) }
        }
    }
}

/// One criterion: its mark (passed, failed, not checked yet) and the checker's note.
private struct CriterionRow: View {
    let item: CriterionResult

    var body: some View {
        HStack(alignment: .top, spacing: 6) {
            Image(systemName: item.symbol)
                .font(.system(size: 13))
                .foregroundStyle(tint)
                .accessibilityLabel(L10n.tr(item.title))
            VStack(alignment: .leading, spacing: 2) {
                Text(item.criterion).font(.callout).textSelection(.enabled)
                if let note = item.check?.note, !note.isEmpty {
                    Text(note).font(.caption).foregroundStyle(Palette.secondary).textSelection(.enabled)
                }
            }
        }
    }

    private var tint: Color {
        guard let check = item.check else { return Palette.secondary }
        return check.pass ? Palette.added : Palette.danger
    }
}

/// Approve the proposed plan, or send it back saying what to change.
private struct PlanDecision: View {
    let task: BoardTask
    let failed: (String) -> Void
    @Environment(BotStore.self) private var model
    @State private var change = ""

    private var trimmed: String { change.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            TextField(L10n.tr("What to change in the plan"), text: $change)
                .textFieldStyle(.plain)
                .fieldBox()
                .submitLabel(.send)
                .onSubmit(ask)
            HStack(spacing: 8) {
                Spacer(minLength: 0)
                Button(L10n.tr("Ask for changes"), action: ask)
                    .buttonStyle(SecondaryButtonStyle())
                    .disabled(trimmed.isEmpty)
                Button(L10n.tr("Approve plan"), action: approve)
                    .buttonStyle(PrimaryButtonStyle())
            }
        }
    }

    private func ask() {
        let note = trimmed
        guard !note.isEmpty else { return }
        Task {
            do {
                try await model.revisePlan(task, note: note)
                change = ""
            } catch {
                failed(error.localizedDescription)
            }
        }
    }

    private func approve() {
        Task {
            do {
                try await model.approvePlan(task)
            } catch {
                failed(error.localizedDescription)
            }
        }
    }
}

/// The questions the claimer asks: answered ones read-only; while the task waits, a field for each open one.
private struct QuestionList: View {
    let task: BoardTask
    let failed: (String) -> Void
    @Environment(BotStore.self) private var model
    @State private var typed: [Int: String] = [:]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(L10n.tr("Questions for you")).font(.caption.weight(.semibold)).foregroundStyle(Palette.secondary)
            ForEach(Array(task.questions.enumerated()), id: \.offset) { index, question in
                VStack(alignment: .leading, spacing: 4) {
                    Text(question.text).font(.callout).textSelection(.enabled)
                    if question.isAnswered {
                        Text(question.answer ?? "").font(.callout).foregroundStyle(Palette.secondary).textSelection(.enabled)
                    } else if task.awaitsAnswers {
                        TextField(L10n.tr("Your answer"), text: answerBinding(index))
                            .textFieldStyle(.plain)
                            .fieldBox()
                    }
                }
            }
            if task.awaitsAnswers {
                HStack(spacing: 8) {
                    Spacer(minLength: 0)
                    Button(L10n.tr("Send answers"), action: send)
                        .buttonStyle(PrimaryButtonStyle())
                        .disabled(task.answers(typed: typed) == nil)
                }
            }
        }
    }

    private func answerBinding(_ index: Int) -> Binding<String> {
        Binding(get: { typed[index] ?? "" }, set: { typed[index] = $0 })
    }

    private func send() {
        guard let answers = task.answers(typed: typed) else { return }
        Task {
            do {
                try await model.answerQuestions(task, answers: answers)
                typed = [:]
            } catch {
                failed(error.localizedDescription)
            }
        }
    }
}
