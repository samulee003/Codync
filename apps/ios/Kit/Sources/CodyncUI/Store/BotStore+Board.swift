import CodyncKit
import Foundation

/// The computer's team board (`host/src/board/`): its tasks come from `tasks` and the `task` events; every change goes
/// through the computer, which answers the task as saved.
extension BotStore {
    /// Whether the computer's host has the board; an older one has no `tasks` method, so the app leaves it out.
    public var hasBoard: Bool { hostVersion.map { !AppVersion.isBelow($0.version, BoardTask.firstHost) } ?? false }

    /// A task from the computer, kept unless an older copy (a lower rev); a deleted one leaves.
    func upsertTask(_ task: BoardTask) {
        if let known = tasks[task.id], known.rev > task.rev { return }
        tasks[task.id] = task.deleted ? nil : task
    }

    func loadBoard() async throws {
        let board = try await withLink(replay: true) { try await $0.tasks() }
        for task in board.tasks { upsertTask(task) }
        boardOptedOut = board.optedOut
    }

    func setBoardClaims(_ botId: String, on: Bool) async throws {
        boardOptedOut = try await withLink { try await $0.setBoardClaims(botId: botId, on: on) }
    }

    /// Answers the name of the bot splitting it.
    func splitGoal(_ goal: String) async throws -> String {
        try await withLink { try await $0.splitGoal(goal) }
    }

    func createTask(title: String, detail: String, project: String?) async throws {
        upsertTask(try await withLink { try await $0.createTask(title: title, detail: detail, project: project) })
    }

    func noteTask(_ task: BoardTask, text: String) async throws {
        upsertTask(try await withLink { try await $0.noteTask(taskId: task.id, text: text) })
    }

    func assignTask(_ task: BoardTask, to botId: String) async throws {
        upsertTask(try await withLink { try await $0.assignTask(taskId: task.id, botId: botId) })
    }

    func perform(_ action: BoardTask.Action, on task: BoardTask) async throws {
        switch action {
        case .assign: return
        case .merge: upsertTask(try await withLink { try await $0.mergeTask(taskId: task.id) })
        case .reopen: upsertTask(try await withLink { try await $0.setTaskStatus(taskId: task.id, status: .open) })
        case .done: upsertTask(try await withLink { try await $0.setTaskStatus(taskId: task.id, status: .done) })
        case .cancel: upsertTask(try await withLink { try await $0.setTaskStatus(taskId: task.id, status: .cancelled) })
        case .delete:
            try await withLink { try await $0.deleteTask(taskId: task.id) }
            tasks[task.id] = nil
        }
    }
}
