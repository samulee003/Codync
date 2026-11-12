import Foundation
import Testing
@testable import CodyncKit

private func taskJSON(_ id: String, status: String, updatedAt: Int64, branch: String? = nil, deleted: Bool = false) -> String {
    let branchValue = branch.map { "\"\($0)\"" } ?? "null"
    return #"{"id":"\#(id)","title":"\#(id)","detail":"","project":null,"status":"\#(status)","createdBy":null,"claimedBy":null,"branch":\#(branchValue),"worktree":null,"notes":[{"by":null,"text":"hi","at":1}],"summary":null,"createdAt":0,"updatedAt":\#(updatedAt),"rev":\#(updatedAt),"deleted":\#(deleted)}"#
}

private func decodeTask(_ json: String) throws -> BoardTask {
    try JSONDecoder().decode(BoardTask.self, from: Data(json.utf8))
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
    let tasks = try [
        decodeTask(taskJSON("a", status: "open", updatedAt: 1)),
        decodeTask(taskJSON("b", status: "open", updatedAt: 3)),
        decodeTask(taskJSON("c", status: "done", updatedAt: 2)),
        decodeTask(taskJSON("d", status: "open", updatedAt: 4, deleted: true)),
    ]
    let columns = BoardTask.columns(tasks)
    #expect(columns.map(\.status) == [.open, .done], "empty columns are left out")
    #expect(columns.first?.tasks.map(\.id) == ["b", "a"])
    #expect(try decodeTask(taskJSON("e", status: "review", updatedAt: 1)).actions == [.done, .reopen, .cancel], "no branch, no merge")
}

@Test func aTaskBeingCheckedNamesItsCheckerAndParent() throws {
    let json = #"{"id":"s","title":"Slides","status":"checking","claimedBy":"ada","reviewer":"bo","parent":"p","notes":[],"createdAt":0,"updatedAt":1,"rev":1}"#
    let task = try decodeTask(json)
    let parent = try decodeTask(#"{"id":"p","title":"Workshop","status":"open","notes":[],"createdAt":0,"updatedAt":1,"rev":1}"#)
    #expect(task.status == .checking && task.actions == [.done, .reopen, .cancel])
    let byline = task.byline(name: { $0.uppercased() }, find: { $0 == "p" ? parent : nil })
    #expect(byline.hasPrefix("ADA · ") && byline.contains("BO") && byline.hasSuffix("↳ Workshop"))
    var claimed = task
    claimed.status = .claimed
    #expect(claimed.actions.first == .assign, "the user can hand a planned task to another bot")
    #expect(claimed.byline(name: { $0 }, find: { _ in nil }) == "ada", "the checker shows only while checking")
}

@Test func aTaskFromAnOlderHostDecodesWithDefaults() throws {
    let task = try decodeTask(#"{"id":"o","title":"Old","status":"claimed","notes":[],"createdAt":0,"updatedAt":1,"rev":1}"#)
    #expect(task.acceptance.isEmpty && task.plan == nil && task.approval == .notNeeded && task.checks.isEmpty)
    #expect(task.questions.isEmpty && task.after.isEmpty && task.assignee == nil && task.turns == 0)
    #expect(task.effort == nil && task.criteria.isEmpty && !task.needsUser)
}

@Test func anUnknownApprovalReadsAsNotNeeded() throws {
    let task = try decodeTask(#"{"id":"u","title":"New","status":"claimed","approval":"someday","notes":[],"createdAt":0,"updatedAt":1,"rev":1}"#)
    #expect(task.approval == .notNeeded && !task.needsUser)
}

@Test func aPlanWaitsForTheUserWithItsCriteriaChecked() throws {
    let plan = try decodeTask(#"""
    {"id":"p1","title":"Plan","status":"claimed","approval":"proposed","plan":"Do X","acceptance":["A","B"],
     "checks":[{"criterion":"A","pass":true,"note":"ok"}],"notes":[],"createdAt":0,"updatedAt":2,"rev":2}
    """#)
    #expect(plan.awaitsApproval && plan.needsUser && !plan.awaitsAnswers)
    #expect(plan.criteria.map { $0.check?.pass } == [true, nil])
    #expect(plan.criteria.first?.symbol == "checkmark.circle.fill" && plan.criteria.last?.symbol == "circle")
    #expect(plan.byline(name: { $0 }, find: { _ in nil }).contains(L10n.tr("Plan waits for you")))
    #expect(BoardTask.columns([plan]).isEmpty, "a task waiting for the user is shown in its own section")
    #expect(BoardTask.waitingForUser([plan]).map(\.id) == ["p1"])
}

@Test func questionsWaitForTheUserAndAnswersAreComplete() throws {
    let task = try decodeTask(#"""
    {"id":"q","title":"Ask","status":"claimed","approval":"asking",
     "questions":[{"text":"Which?","answer":null},{"text":"Done?","answer":"yes"}],"notes":[],"createdAt":0,"updatedAt":1,"rev":1}
    """#)
    #expect(task.awaitsAnswers && task.needsUser && !task.awaitsApproval)
    #expect(task.questions.last?.isAnswered == true && task.questions.first?.isAnswered == false)
    #expect(task.answers(typed: [0: "blue"]) == ["blue", "yes"])
    #expect(task.answers(typed: [0: "  "]) == nil, "a blank answer is not sent")
    #expect(task.answers(typed: [:]) == nil)
}

@Test func anOpenTaskWaitsForUnfinishedTasksAndNamesItsAssignee() throws {
    let waiting = try decodeTask(#"{"id":"w","title":"Wait","status":"open","assignee":"bo","after":["x","d"],"notes":[],"createdAt":0,"updatedAt":1,"rev":1}"#)
    let build = try decodeTask(taskJSON("x", status: "open", updatedAt: 1).replacingOccurrences(of: "\"title\":\"x\"", with: "\"title\":\"Build\""))
    let done = try decodeTask(taskJSON("d", status: "done", updatedAt: 1))
    let board = ["x": build, "d": done]
    #expect(waiting.waitingFor { board[$0] } == ["Build"], "a finished task is not waited for")
    let line = waiting.byline(name: { $0.uppercased() }, find: { board[$0] })
    #expect(line.contains("Build") && line.contains("BO"))
    #expect(BoardTask.columns([waiting]).map(\.status) == [.open], "an open task with nothing needed is a normal column")
}

@Test func effortIsTurnsAndMinutesBetweenStartAndFinish() throws {
    let task = try decodeTask(#"{"id":"e","title":"Work","status":"done","startedAt":0,"finishedAt":420000,"turns":3,"notes":[],"createdAt":0,"updatedAt":1,"rev":1}"#)
    #expect(task.effort == TaskEffort(turns: 3, minutes: 7))
    let untimed = try decodeTask(#"{"id":"n","title":"Work","status":"done","turns":0,"notes":[],"createdAt":0,"updatedAt":1,"rev":1}"#)
    #expect(untimed.effort == nil)
}

@Test func criteriaTypedOnePerLineAreParsed() {
    #expect(BoardTask.parseCriteria("- Tests pass\n\n* Lint clean\n  • Docs  \n") == ["Tests pass", "Lint clean", "Docs"])
}
