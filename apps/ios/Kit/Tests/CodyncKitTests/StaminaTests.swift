import Foundation
import Testing
@testable import CodyncKit

@Test func readsEachAgentsStaminaAndItsColor() throws {
    let json = #"{"providers":[],"stamina":{"codex":{"left":33,"label":"Weekly","resetsAt":5},"claude":{"left":80,"label":"5h"},"cursor":{"left":10,"label":"Weekly","resetsText":"Sep 26"}}}"#
    let usage = try JSONDecoder().decode(Usage.self, from: Data(json.utf8))
    #expect(usage.stamina?["codex"]?.left == 33 && usage.stamina?["codex"]?.resetsAt == 5)
    #expect(usage.stamina?["codex"]?.level == .low)
    #expect(usage.stamina?["claude"]?.level == .full)
    #expect(usage.stamina?["cursor"]?.level == .out)
    // A host before the bars sends none.
    #expect(try JSONDecoder().decode(Usage.self, from: Data(#"{"providers":[]}"#.utf8)).stamina == nil)
}
