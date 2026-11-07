import Foundation

/// The team board's methods (`host/src/api/board.rs`).
public extension HostClient {
    private struct TaskAnswer: Decodable { var task: BoardTask }

    func tasks() async throws -> [BoardTask] {
        struct Res: Decodable { var tasks: [BoardTask] }
        let res: Res = try await call("tasks")
        return res.tasks
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
