import Foundation
import Testing
@testable import Stats

@Suite("Clock preferences", .serialized)
struct ClockPreferencesTests {
  @Test("Saved choices preserve their order")
  func savedChoicesPreserveOrder() {
    let defaults = isolatedDefaults()
    defer { defaults.removePersistentDomain(forName: suiteName) }
    let preferences = ClockPreferences(defaults: defaults)
    let choices = [ClockChoice.defaults[2], ClockChoice.defaults[0]]

    preferences.save(choices)

    #expect(preferences.selectedChoices.map(\.id).prefix(2) == choices.map(\.id)[...])
  }

  @Test("Missing preferences receive four defaults")
  func missingPreferencesReceiveDefaults() {
    let defaults = isolatedDefaults()
    defer { defaults.removePersistentDomain(forName: suiteName) }

    let choices = ClockPreferences(defaults: defaults).selectedChoices

    #expect(choices.map(\.id) == ClockChoice.defaults.map(\.id))
  }

  private let suiteName = "StatsTests.ClockPreferences"

  private func isolatedDefaults() -> UserDefaults {
    let defaults = UserDefaults(suiteName: suiteName)!
    defaults.removePersistentDomain(forName: suiteName)
    return defaults
  }
}
