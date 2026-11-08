import CodyncKit
import SwiftUI

/// The computer's team board (`host/src/board/`): tasks the user and the bots share, by status. The user adds tasks,
/// hands one to a bot, merges a finished task's branch, and reopens, closes or deletes tasks; bots claim and finish
/// them through their own tools.
public struct BoardView: View {
    @Environment(BotStore.self) private var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var adding = false
    @State private var splitting = false
    @State private var notice: String?
    @State private var expanded: String?
    @State private var loaded = false
    @State private var error: String?

    public init() {}

    public var body: some View {
        let columns = BoardTask.columns(model.tasks.values)
        VStack(spacing: 0) {
            ModalHeader("Team board") {
                IconButton("Split a goal into tasks", systemImage: "wand.and.stars") { splitting = true }
                IconButton("New task", systemImage: "plus") { adding = true }
            }
            CardForm {
                Text(L10n.tr("Tasks your bots split, claim and finish together"))
                    .font(.callout).foregroundStyle(Palette.secondary)
                if let error {
                    Text(L10n.tr(error)).font(.caption).foregroundStyle(Palette.danger).textSelection(.enabled)
                }
                if let notice {
                    Text(notice).font(.caption).foregroundStyle(Palette.secondary)
                }
                if !columns.isEmpty {
                    ForEach(columns) { column in
                        CardSection("\(L10n.tr(column.status.title)) · \(column.tasks.count)") {
                            ForEach(column.tasks) { row($0) }
                        }
                    }
                } else if loaded {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(L10n.tr("No tasks yet")).font(.callout.weight(.semibold))
                        Text(L10n.tr("Add a task with +, or split a whole goal into tasks with the wand."))
                            .font(.callout).foregroundStyle(Palette.secondary)
                    }
                    .outlinedCard()
                } else {
                    Spinner(size: 20).frame(maxWidth: .infinity).padding(.top, 40)
                }
                BoardMembers { self.error = $0 }
            }
        }
        .background(Palette.background)
        .animation(Motion.reduced(Motion.layout, reduceMotion), value: expanded)
        .tracked(.screenViewed(.board))
        .task {
            do {
                try await model.loadBoard()
            } catch {
                self.error = error.localizedDescription
            }
            loaded = true
        }
        .codyncSheet(isPresented: $adding) { NewTaskSheet() }
        .codyncSheet(isPresented: $splitting) {
            SplitGoalSheet { name in notice = L10n.tr("{name} is splitting it into tasks.").replacingOccurrences(of: "{name}", with: name) }
        }
    }

    private func row(_ task: BoardTask) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                Button {
                    expanded = expanded == task.id ? nil : task.id
                } label: {
                    HStack(spacing: 10) {
                        Image(systemName: task.status.symbol).font(.system(size: 15)).foregroundStyle(Palette.secondary)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(task.title).font(.body.weight(.medium)).foregroundStyle(Palette.text).lineLimit(2)
                            if !subtitle(task).isEmpty {
                                Text(subtitle(task)).font(.caption).foregroundStyle(Palette.secondary).lineLimit(1)
                            }
                        }
                        Spacer(minLength: 0)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                DropdownMenu(items: { menuItems(task) }) {
                    Image(systemName: "ellipsis").font(.system(size: 15)).foregroundStyle(Palette.secondary)
                        .frame(width: 32, height: 32).contentShape(Rectangle())
                }
                .accessibilityLabel(L10n.tr("Task actions"))
            }
            if expanded == task.id {
                TaskDetail(task: task) { self.error = $0 }
                    .transition(.opacity)
            }
        }
        .outlinedCard(vertical: 10)
    }

    /// Who holds it, its branch, and how many notes it has.
    private func subtitle(_ task: BoardTask) -> String {
        var parts: [String] = []
        if let bot = task.claimedBy { parts.append(model.bots[bot]?.name ?? bot) }
        if let branch = task.branch { parts.append(branch) }
        if !task.notes.isEmpty { parts.append(L10n.tr("{count} notes").replacingOccurrences(of: "{count}", with: "\(task.notes.count)")) }
        return parts.joined(separator: " · ")
    }

    private func menuItems(_ task: BoardTask) -> [MenuItem] {
        task.actions.flatMap { action -> [MenuItem] in
            guard action != .assign else {
                return model.bots.values.filter { !$0.isGroup && !$0.hidden }.sorted { $0.name < $1.name }.map { bot in
                    MenuItem(L10n.tr("Assign to {name}").replacingOccurrences(of: "{name}", with: bot.name), icon: "person.crop.circle.badge.plus",
                             verbatim: true) { run { try await model.assignTask(task, to: bot.id) } }
                }
            }
            let (title, icon, destructive) = Self.label(action)
            return [MenuItem(title, icon: icon, destructive: destructive) { run { try await model.perform(action, on: task) } }]
        }
    }

    private static func label(_ action: BoardTask.Action) -> (String, String, Bool) {
        switch action {
        case .assign: ("Assign to {name}", "person.crop.circle.badge.plus", false)
        case .merge: ("Merge branch", "arrow.triangle.merge", false)
        case .reopen: ("Reopen", "arrow.uturn.backward", false)
        case .done: ("Mark done", "checkmark", false)
        case .cancel: ("Cancel task", "xmark", false)
        case .delete: ("Delete task", "trash", true)
        }
    }

    private func run(_ work: @escaping @MainActor () async throws -> Void) {
        Task {
            do {
                try await work()
                error = nil
            } catch {
                self.error = error.localizedDescription
            }
        }
    }
}

/// An unfolded task: its detail, where its work is, its summary and notes, and a field to add a note.
private struct TaskDetail: View {
    let task: BoardTask
    let failed: (String) -> Void
    @Environment(BotStore.self) private var model
    @State private var note = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if !task.detail.isEmpty { Text(task.detail).font(.callout).textSelection(.enabled) }
            if let place = task.worktree ?? task.project {
                Text(place).font(.caption.monospaced()).foregroundStyle(Palette.secondary).lineLimit(1).truncationMode(.middle)
            }
            if let summary = task.summary { Text(summary).font(.callout).textSelection(.enabled) }
            ForEach(task.notes, id: \.self) { n in
                Text("\(n.by.map { model.bots[$0]?.name ?? $0 } ?? L10n.tr("You")): \(n.text)")
                    .font(.callout).foregroundStyle(Palette.secondary).textSelection(.enabled)
            }
            TextField(L10n.tr("Add a note for the team"), text: $note)
                .textFieldStyle(.plain)
                .fieldBox()
                .submitLabel(.send)
                .onSubmit(send)
        }
        .padding(.leading, 25)
    }

    private func send() {
        let text = note.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        Task {
            do {
                try await model.noteTask(task, text: text)
                note = ""
            } catch {
                failed(error.localizedDescription)
            }
        }
    }
}
