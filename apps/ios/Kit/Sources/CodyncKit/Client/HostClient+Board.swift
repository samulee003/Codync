import Foundation

/// The team board's methods (`host/src/api/board.rs`).
public extension HostClient {
    private struct TaskAnswer: Decodable { var task: BoardTask }

    /// The board, with the bots turned off from claiming tasks on their own (none from hosts before it).
    func tasks() async throws -> (tasks: [BoardTask], optedOut: Set<String>) {
        struct Res: Decodable { var tasks: [BoardTask]; var optedOut: [String]? }
        let res: Res = try await call("tasks")
        return (res.tasks, Set(res.optedOut ?? []))
    }

    func setBoardClaims(botId: String, on: Bool) async throws -> Set<String> {
        struct Body: Encodable { var botId: String; var on: Bool }
        struct Res: Decodable { var optedOut: [String] }
        let res: Res = try await call("setBoardClaims", Body(botId: botId, on: on))
        return Set(res.optedOut)
    }

    /// Hands the goal to the lead bot, which splits it into tasks; answers its name.
    func splitGoal(_ goal: String) async throws -> String {
        struct Res: Decodable { var name: String }
        let res: Res = try await call("splitGoal", ["goal": goal])
        return res.name
    }

    func createTask(title: String, detail: String, project: String?) async throws -> BoardTask {
        struct Body: Encodable { var title: String; var detail: String; var project: String? }
        let res: TaskAnswer = try await call("createTask", Body(title: title, detail: detail, project: project))
        return res.task
    }

    func noteTask(taskId: String, text: String) async throws -> BoardTask {
        let res: TaskAnswer = try await call("noteTask", ["taskId": taskId, "text": text])
        return res.task
    }

    func assignTask(taskId: String, botId: String) async throws -> BoardTask {
        let res: TaskAnswer = try await call("assignTask", ["taskId": taskId, "botId": botId])
        return res.task
    }

    func setTaskStatus(taskId: String, status: BoardTask.Status) async throws -> BoardTask {
        let res: TaskAnswer = try await call("setTaskStatus", ["taskId": taskId, "status": status.rawValue])
        return res.task
    }

    /// Merging runs git in the project, which can take a while on a large repository.
    func mergeTask(taskId: String) async throws -> BoardTask {
        let res: TaskAnswer = try await call("mergeTask", ["taskId": taskId], timeout: 120)
        return res.task
    }

    func deleteTask(taskId: String) async throws {
        struct Res: Decodable { var ok: Bool }
        let _: Res = try await call("deleteTask", ["taskId": taskId])
    }
}
