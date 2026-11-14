import CodyncKit
import SwiftUI

/// A new task: its title, what to do, and the folder it's in, picked like a bot's workspace (a git project gives each
/// claim its own worktree).
struct NewTaskSheet: View {
    @Environment(BotStore.self) private var model
    @Environment(\.dismissModal) private var dismiss
    @State private var title = ""
    @State private var detail = ""
    @State private var criteria = ""
    @State private var approvePlan = true
    @State private var project: String?
    @State private var picking = false
    @State private var saving = false
    @State private var error: String?
    @FocusState private var focused: Bool

    private var trimmed: String { title.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        VStack(spacing: 0) {
            ModalHeader("New task") {
                if saving {
                    Spinner()
                } else {
                    IconButton("Add task", systemImage: "checkmark", action: save).disabled(trimmed.isEmpty)
                }
            }
            CardForm {
                CardSection {
                    TextField(L10n.tr("Task title"), text: $title).focused($focused)
                    TextField(L10n.tr("What to do: the result, the materials to use, and what not to do"), text: $detail, axis: .vertical)
                        .lineLimit(3...8)
                    TextField(L10n.tr("Acceptance criteria, one per line: what must hold for it to count as done"), text: $criteria, axis: .vertical)
                        .lineLimit(3...8)
                    HStack(spacing: 8) {
                        Button { picking = true } label: {
                            HStack(spacing: 8) {
                                Image(systemName: "folder").foregroundStyle(Palette.secondary)
                                Text(project ?? L10n.tr("Project folder (optional)"))
                                    .font(project == nil ? .body : .body.monospaced())
                                    .foregroundStyle(project == nil ? Palette.secondary : Palette.text)
                                    .lineLimit(1).truncationMode(.middle)
                                Spacer(minLength: 0)
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        if project != nil {
                            IconButton("Clear", systemImage: "xmark") { project = nil }
                        }
                    }
                    Toggle(isOn: $approvePlan) {
                        Text(L10n.tr("Approve the plan before work starts"))
                    }
                    .toggleStyle(.codync)
                }
                if let error {
                    Text(L10n.tr(error)).font(.caption).foregroundStyle(Palette.danger).textSelection(.enabled)
                }
            }
            .textFieldStyle(.plain)
        }
        .background(Palette.background)
        .presentationDetents([.medium, .large])
        .onAppear { focused = true }
        .codyncSheet(isPresented: $picking) {
            FolderPicker(start: project ?? model.hello?.home) {
                project = $0
                picking = false
            }
        }
    }

    private func save() {
        guard !saving, !trimmed.isEmpty else { return }
        saving = true
        Task {
            do {
                try await model.createTask(title: trimmed, detail: detail, project: project,
                                           acceptance: BoardTask.parseCriteria(criteria), approvePlan: approvePlan)
                dismiss()
            } catch {
                self.error = error.localizedDescription
            }
            saving = false
        }
    }
}

/// A whole goal, for the lead bot to split into tasks the bots then claim. `handed` gets the splitter's name.
struct SplitGoalSheet: View {
    let handed: (String) -> Void
    @Environment(BotStore.self) private var model
    @Environment(\.dismissModal) private var dismiss
    @State private var goal = ""
    @State private var saving = false
    @State private var error: String?
    @FocusState private var focused: Bool

    private var trimmed: String { goal.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        VStack(spacing: 0) {
            ModalHeader("Split a goal into tasks") {
                if saving {
                    Spinner()
                } else {
                    IconButton("Split into tasks", systemImage: "checkmark", action: split).disabled(trimmed.isEmpty)
                }
            }
            CardForm {
                CardSection(footer: "Your lead bot splits it into tasks, and the bots claim them.") {
                    TextField(L10n.tr("The goal, e.g. prepare both kids’ exam review papers"), text: $goal, axis: .vertical)
                        .lineLimit(3...10)
                        .focused($focused)
                }
                if let error {
                    Text(L10n.tr(error)).font(.caption).foregroundStyle(Palette.danger).textSelection(.enabled)
                }
            }
            .textFieldStyle(.plain)
        }
        .background(Palette.background)
        .presentationDetents([.medium, .large])
        .onAppear { focused = true }
    }

    private func split() {
        guard !saving, !trimmed.isEmpty else { return }
        saving = true
        Task {
            do {
                handed(try await model.splitGoal(trimmed))
                dismiss()
            } catch {
                self.error = error.localizedDescription
            }
            saving = false
        }
    }
}

/// Which bots come for open tasks on their own: all, unless turned off here.
struct BoardMembers: View {
    let failed: (String) -> Void
    @Environment(BotStore.self) private var model

    var body: some View {
        let bots = model.bots.values.filter { !$0.isGroup && !$0.hidden }.sorted { $0.name < $1.name }
        if !bots.isEmpty {
            CardSection("Bots that claim tasks on their own") {
                ForEach(bots) { bot in
                    Toggle(isOn: Binding(get: { !model.boardOptedOut.contains(bot.id) }, set: { on in set(bot.id, on) })) {
                        Text(bot.name)
                    }
                    .toggleStyle(.codync)
                }
            }
        }
    }

    private func set(_ bot: String, _ on: Bool) {
        Task {
            do {
                try await model.setBoardClaims(bot, on: on)
            } catch {
                failed(error.localizedDescription)
            }
        }
    }
}
