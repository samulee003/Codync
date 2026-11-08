import Foundation
import Testing
@testable import CodyncKit

private func taskJSON(_ id: String, status: String, updatedAt: Int64, branch: String? = nil, deleted: Bool = false) -> String {
    let branchValue = branch.map { "\"\($0)\"" } ?? "null"
    return #"{"id":"\#(id)","title":"\#(id)","detail":"","project":null,"status":"\#(status)","createdBy":null,"claimedBy":null,"branch":\#(branchValue),"worktree":null,"notes":[{"by":null,"text":"hi","at":1}],"summary":null,"createdAt":0,"updatedAt":\#(updatedAt),"rev":\#(updatedAt),"deleted":\#(deleted)}"#
}

@Test func parsesTaskEventsAndSkipsWhatItCantRead() throws {
    let event = HostClient.parseEvent(Data(#"{"type":"task","rev":3,"task":\#(taskJSON("t1", status: "review", updatedAt: 3, branch: "codync/task-t1"))}"#.utf8))
    guard case let .task(task) = event else { Issue.record("task"); return }
    #expect(task.status == .review && task.branch == "codync/task-t1" && task.notes.first?.text == "hi")
    #expect(task.actions == [.merge, .reopen, .cancel])
    // A task this app can't read is skipped, not a reason to fetch everything again.
    #expect(HostClient.parseEvent(Data(#"{"type":"task","task":{"id":"t1"}}"#.utf8)) == nil)
}

@Test func columnsFollowTheStatusOrderNewestFirst() throws {
    let decode = { (json: String) in try JSONDecoder().decode(BoardTask.self, from: Data(json.utf8)) }
    let tasks = try [
        decode(taskJSON("a", status: "open", updatedAt: 1)),
        decode(taskJSON("b", status: "open", updatedAt: 3)),
        decode(taskJSON("c", status: "done", updatedAt: 2)),
        decode(taskJSON("d", status: "open", updatedAt: 4, deleted: true)),
    ]
    let columns = BoardTask.columns(tasks)
    #expect(columns.map(\.status) == [.open, .done], "empty columns are left out")
    #expect(columns.first?.tasks.map(\.id) == ["b", "a"])
    #expect(try decode(taskJSON("e", status: "review", updatedAt: 1)).actions == [.done, .reopen, .cancel], "no branch, no merge")
}

@Test func aTaskBeingCheckedNamesItsCheckerAndParent() throws {
    let json = #"{"id":"s","title":"Slides","status":"checking","claimedBy":"ada","reviewer":"bo","parent":"p","notes":[],"createdAt":0,"updatedAt":1,"rev":1}"#
    let task = try JSONDecoder().decode(BoardTask.self, from: Data(json.utf8))
    #expect(task.status == .checking && task.actions == [.done, .reopen, .cancel])
    let byline = task.byline(name: { $0.uppercased() }, parentTitle: { $0 == "p" ? "Workshop" : nil })
    #expect(byline.hasPrefix("ADA · ") && byline.contains("BO") && byline.hasSuffix("↳ Workshop"))
    var claimed = task
    claimed.status = .claimed
    #expect(claimed.actions.first == .assign, "the user can hand a planned task to another bot")
    #expect(claimed.byline(name: { $0 }, parentTitle: { _ in nil }) == "ada", "the checker shows only while checking")
}
