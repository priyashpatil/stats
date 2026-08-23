import Foundation
import Testing
@testable import Stats

@Suite("Clock choices")
struct ClockChoiceTests {
  @Test("Legacy time zones use their preferred city labels")
  func legacyTimezoneLabel() throws {
    let choice = try #require(ClockChoice.choice(forLegacyTimezone: "America/Los_Angeles"))

    #expect(choice.label == "Seattle")
    #expect(choice.timezone == "America/Los_Angeles")
  }

  @Test("Clock identifiers distinguish cities sharing a time zone")
  func identifierIncludesLabel() {
    let seattle = ClockChoice(label: "Seattle", timezone: "America/Los_Angeles")
    let portland = ClockChoice(label: "Portland", timezone: "America/Los_Angeles")

    #expect(seattle.id != portland.id)
  }
}
