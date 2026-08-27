import Foundation
import Testing

@testable import Stats

@Suite("Clock choices")
struct ClockChoiceTests {
  @Test("Clock identifiers distinguish cities sharing a time zone")
  func identifierIncludesLabel() {
    let seattle = ClockChoice(label: "Seattle", timezone: "America/Los_Angeles")
    let portland = ClockChoice(label: "Portland", timezone: "America/Los_Angeles")

    #expect(seattle.id != portland.id)
  }
}
