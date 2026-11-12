import Foundation

/// A task on the computer's team board (`host/src/board/`): the user and the bots add tasks, a bot claims one, notes
/// what it finds and finishes it. A task in a git project gets its own worktree and branch when claimed.
public struct BoardTask: Codable, Hashable, Sendable, Identifiable {
    public enum Status: String, Codable, Sendable, CaseIterable {
        case open, claimed, checking, review, done, cancelled

        /// The board's heading for the status.
        public var title: String {
            switch self {
            case .open: "Open"
            case .claimed: "In progress"
            case .checking: "Being checked"
            case .review: "Ready to merge"
            case .done: "Done"
            case .cancelled: "Cancelled"
            }
        }

        public var symbol: String {
            switch self {
            case .open: "circle"
            case .claimed: "circle.dotted"
            case .checking: "checkmark.seal"
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
    /// The task this one is a part of (a subtask).
    public var parent: String?
    /// The bot that checks (or checked) the finished work.
    public var reviewer: String?
    public var createdAt: Int64
    public var updatedAt: Int64
    public var rev: Int64
    public var deleted: Bool
    /// What must hold for the result to count as done; the checker marks each criterion pass or fail.
    public var acceptance: [String]
    /// The plan its claimer proposed, or the approved one.
    public var plan: String?
    public var approval: PlanApproval
    /// The checker's latest verdict on each criterion.
    public var checks: [CriterionCheck]
    /// The questions the claimer asks the user, with the answers given.
    public var questions: [TaskQuestion]
    /// The tasks it waits for: it starts once they are finished.
    public var after: [String]
    /// The bot it goes to once nothing it waits for is left.
    public var assignee: String?
    /// Milliseconds since the epoch, when the claimer started and finished it.
    public var startedAt: Int64?
    public var finishedAt: Int64?
    public var turns: Int

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
        parent = try c.decodeIfPresent(String.self, forKey: .parent)
        reviewer = try c.decodeIfPresent(String.self, forKey: .reviewer)
        createdAt = try c.decodeIfPresent(Int64.self, forKey: .createdAt) ?? 0
        updatedAt = try c.decodeIfPresent(Int64.self, forKey: .updatedAt) ?? 0
        rev = try c.decodeIfPresent(Int64.self, forKey: .rev) ?? 0
        deleted = try c.decodeIfPresent(Bool.self, forKey: .deleted) ?? false
        // Fields from newer hosts: an absent or unreadable one takes its default and never fails the task.
        acceptance = (try? c.decodeIfPresent([String].self, forKey: .acceptance)) ?? []
        plan = try? c.decodeIfPresent(String.self, forKey: .plan)
        approval = (try? c.decodeIfPresent(String.self, forKey: .approval)).flatMap(PlanApproval.init(rawValue:)) ?? .notNeeded
        checks = (try? c.decodeIfPresent([CriterionCheck].self, forKey: .checks)) ?? []
        questions = (try? c.decodeIfPresent([TaskQuestion].self, forKey: .questions)) ?? []
        after = (try? c.decodeIfPresent([String].self, forKey: .after)) ?? []
        assignee = try? c.decodeIfPresent(String.self, forKey: .assignee)
        startedAt = try? c.decodeIfPresent(Int64.self, forKey: .startedAt)
        finishedAt = try? c.decodeIfPresent(Int64.self, forKey: .finishedAt)
        turns = (try? c.decodeIfPresent(Int.self, forKey: .turns)) ?? 0
    }

    /// The actions its status allows, in menu order.
    public var actions: [Action] {
        switch status {
        case .open: [.assign, .done, .cancel, .delete]
        case .claimed: [.assign, .reopen, .cancel]
        case .checking: [.done, .reopen, .cancel]
        case .review: branch == nil ? [.done, .reopen, .cancel] : [.merge, .reopen, .cancel]
        case .done, .cancelled: [.reopen, .delete]
        }
    }

    /// The plan waits for the user's approval.
    public var awaitsApproval: Bool { status == .claimed && approval == .proposed }

    /// The claimer has questions for the user.
    public var awaitsAnswers: Bool { status == .claimed && approval == .asking }

    /// The user has to act on it: approve the plan or answer the questions.
    public var needsUser: Bool { awaitsApproval || awaitsAnswers }

    /// Done, cancelled, or finished with no branch left to merge: out of the way of a task that waits for it.
    public var finishedForOthers: Bool {
        status == .done || status == .cancelled || (status == .review && branch == nil)
    }

    /// Each acceptance criterion with the latest check on it.
    public var criteria: [CriterionResult] {
        acceptance.enumerated().map { index, criterion in
            CriterionResult(id: index, criterion: criterion, check: checks.first { $0.criterion == criterion })
        }
    }

    /// The titles of the unfinished tasks an open task still waits for.
    public func waitingFor(_ find: (String) -> BoardTask?) -> [String] {
        guard status == .open else { return [] }
        return after.compactMap(find).filter { !$0.deleted && !$0.finishedForOthers }.map(\.title)
    }

    /// The work spent on it: its turns and the minutes from its start to its finish; nil when there is none.
    public var effort: TaskEffort? {
        var minutes = 0
        if let startedAt, let finishedAt, finishedAt >= startedAt { minutes = Int((finishedAt - startedAt) / 60_000) }
        guard turns > 0 || minutes > 0 else { return nil }
        return TaskEffort(turns: turns, minutes: minutes)
    }

    /// The answers to send: the given ones, and what the user typed for each open question; nil while one is blank.
    public func answers(typed: [Int: String]) -> [String]? {
        var result: [String] = []
        for (index, question) in questions.enumerated() {
            if question.isAnswered {
                result.append(question.answer ?? "")
                continue
            }
            let text = (typed[index] ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            guard !text.isEmpty else { return nil }
            result.append(text)
        }
        return result
    }

    /// Its byline, as on the desktop: what it waits for, its assignee, who holds it, what the user has to do, who checks
    /// it, the task it is part of, its branch and how many notes it has.
    public func byline(name: (String) -> String, find: (String) -> BoardTask?) -> String {
        var parts: [String] = []
        let waiting = waitingFor(find)
        if !waiting.isEmpty {
            parts.append(L10n.tr("Waits for {titles}").replacingOccurrences(of: "{titles}", with: waiting.joined(separator: ", ")))
        }
        if status == .open, let assignee {
            parts.append(L10n.tr("For {name}").replacingOccurrences(of: "{name}", with: name(assignee)))
        }
        if let claimedBy { parts.append(name(claimedBy)) }
        if awaitsApproval { parts.append(L10n.tr("Plan waits for you")) }
        if awaitsAnswers { parts.append(L10n.tr("Has questions for you")) }
        if status == .checking, let reviewer {
            parts.append(L10n.tr("Checked by {name}").replacingOccurrences(of: "{name}", with: name(reviewer)))
        }
        if let parent, let title = find(parent)?.title { parts.append("↳ \(title)") }
        if let branch { parts.append(branch) }
        if !notes.isEmpty { parts.append(L10n.tr("{count} notes").replacingOccurrences(of: "{count}", with: "\(notes.count)")) }
        return parts.joined(separator: " · ")
    }

    /// The first host with the team board.
    public static let firstHost = "2.41.0"

    /// One status's tasks on the board.
    public struct Column: Identifiable, Sendable {
        public var status: Status
        public var tasks: [BoardTask]
        public var id: Status { status }
    }

    /// The board's columns: each status with its tasks, newest first; empty columns left out. Tasks that wait for the
    /// user are shown by `waitingForUser` instead.
    public static func columns(_ tasks: some Sequence<BoardTask>) -> [Column] {
        let live = tasks.filter { !$0.deleted && !$0.needsUser }.sorted { $0.updatedAt > $1.updatedAt }
        return Status.allCases.compactMap { status in
            let column = live.filter { $0.status == status }
            return column.isEmpty ? nil : Column(status: status, tasks: column)
        }
    }

    /// The tasks that wait for the user (a plan to approve, questions to answer), newest first.
    public static func waitingForUser(_ tasks: some Sequence<BoardTask>) -> [BoardTask] {
        tasks.filter { !$0.deleted && $0.needsUser }.sorted { $0.updatedAt > $1.updatedAt }
    }
}
