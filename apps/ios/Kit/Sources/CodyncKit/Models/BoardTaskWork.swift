import Foundation

/// Whether the user approves the claimer's plan before work starts. `asking`: the claimer has questions for the user
/// instead. An unknown value reads as `notNeeded` (see `BoardTask.init(from:)`).
public enum PlanApproval: String, Codable, Sendable {
    case notNeeded, waiting, asking, proposed, approved
}

/// The checker's verdict on one acceptance criterion.
public struct CriterionCheck: Codable, Hashable, Sendable {
    public var criterion: String
    public var pass: Bool
    public var note: String

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        criterion = try c.decode(String.self, forKey: .criterion)
        pass = try c.decode(Bool.self, forKey: .pass)
        note = try c.decodeIfPresent(String.self, forKey: .note) ?? ""
    }
}

/// A question the claimer asks the user; `answer` is set once the user has answered it.
public struct TaskQuestion: Codable, Hashable, Sendable {
    public var text: String
    public var answer: String?

    /// Answered with something to show (an empty answer still waits).
    public var isAnswered: Bool { !(answer ?? "").isEmpty }
}

/// One acceptance criterion with the latest check on it, if it was checked.
public struct CriterionResult: Identifiable, Hashable, Sendable {
    public var id: Int
    public var criterion: String
    public var check: CriterionCheck?

    /// The mark it shows: passed, failed, or not checked yet.
    public var symbol: String {
        guard let check else { return "circle" }
        return check.pass ? "checkmark.circle.fill" : "xmark.circle.fill"
    }

    /// Its label, matching the mark (translated by the view).
    public var title: String {
        guard let check else { return "Not checked yet" }
        return check.pass ? "Passed" : "Failed"
    }
}

/// The work spent on a task: its turns, and the minutes from its start to its finish.
public struct TaskEffort: Hashable, Sendable {
    public var turns: Int
    public var minutes: Int

    /// Its line on a task card.
    public var line: String {
        L10n.tr("{turns} turns · {minutes} min")
            .replacingOccurrences(of: "{turns}", with: "\(turns)")
            .replacingOccurrences(of: "{minutes}", with: "\(minutes)")
    }
}

public extension BoardTask {
    /// Acceptance criteria typed one per line: list marks and leading spaces dropped, blank lines left out.
    static func parseCriteria(_ text: String) -> [String] {
        text.components(separatedBy: .newlines)
            .map { line in
                String(line.drop(while: { ch in ch.isWhitespace || "-*•".contains(ch) })).trimmingCharacters(in: .whitespaces)
            }
            .filter { !$0.isEmpty }
    }
}
