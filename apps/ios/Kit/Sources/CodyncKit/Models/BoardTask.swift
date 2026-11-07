import Foundation

/// A task on the computer's team board (`host/src/board/`): the user and the bots add tasks, a bot claims one, notes
/// what it finds and finishes it. A task in a git project gets its own worktree and branch when claimed.
public struct BoardTask: Codable, Hashable, Sendable, Identifiable {
    public enum Status: String, Codable, Sendable, CaseIterable {
        case open, claimed, review, done, cancelled

        /// The board's heading for the status.
        public var title: String {
            switch self {
            case .open: "Open"
            case .claimed: "In progress"
            case .review: "Ready to merge"
            case .done: "Done"
            case .cancelled: "Cancelled"
            }
        }

        public var symbol: String {
            switch self {
            case .open: "circle"
            case .claimed: "circle.dotted"
            case .review: "arrow.triangle.merge"
            case .done: "checkmark.circle.fill"
            case .cancelled: "xmark.circle"
            }
        }
    }

    public struct Note: Codable, Hashable, Sendable {
        /// The bot that wrote it; nil: the user.
        public var by: String?
        public var text: String
        public var at: Int64
    }

    /// What the user can do with a task in its status.
    public enum Action: String, Sendable {
        case assign, merge, reopen, done, cancel, delete
    }

    public var id: String
    public var title: String
    public var detail: String
    public var project: String?
    public var status: Status
    public var createdBy: String?
    public var claimedBy: String?
    public var branch: String?
    public var worktree: String?
    public var notes: [Note]
    public var summary: String?
    public var createdAt: Int64
    public var updatedAt: Int64
    public var rev: Int64
    public var deleted: Bool

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id)
        title = try c.decode(String.self, forKey: .title)
        detail = try c.decodeIfPresent(String.self, forKey: .detail) ?? ""
        project = try c.decodeIfPresent(String.self, forKey: .project)
        status = try c.decode(Status.self, forKey: .status)
        createdBy = try c.decodeIfPresent(String.self, forKey: .createdBy)
        claimedBy = try c.decodeIfPresent(String.self, forKey: .claimedBy)
        branch = try c.decodeIfPresent(String.self, forKey: .branch)
        worktree = try c.decodeIfPresent(String.self, forKey: .worktree)
        notes = try c.decodeIfPresent([Note].self, forKey: .notes) ?? []
        summary = try c.decodeIfPresent(String.self, forKey: .summary)
        createdAt = try c.decodeIfPresent(Int64.self, forKey: .createdAt) ?? 0
        updatedAt = try c.decodeIfPresent(Int64.self, forKey: .updatedAt) ?? 0
        rev = try c.decodeIfPresent(Int64.self, forKey: .rev) ?? 0
        deleted = try c.decodeIfPresent(Bool.self, forKey: .deleted) ?? false
    }

    /// The actions its status allows, in menu order.
    public var actions: [Action] {
        switch status {
        case .open: [.assign, .done, .cancel, .delete]
        case .claimed: [.reopen, .cancel]
        case .review: branch == nil ? [.done, .reopen, .cancel] : [.merge, .reopen, .cancel]
        case .done, .cancelled: [.reopen, .delete]
        }
    }

    /// The first host with the team board.
    public static let firstHost = "2.41.0"

    /// One status's tasks on the board.
    public struct Column: Identifiable, Sendable {
        public var status: Status
        public var tasks: [BoardTask]
        public var id: Status { status }
    }

    /// The board's columns: each status with its tasks, newest first; empty columns left out.
    public static func columns(_ tasks: some Sequence<BoardTask>) -> [Column] {
        let live = tasks.filter { !$0.deleted }.sorted { $0.updatedAt > $1.updatedAt }
        return Status.allCases.compactMap { status in
            let column = live.filter { $0.status == status }
            return column.isEmpty ? nil : Column(status: status, tasks: column)
        }
    }
}
