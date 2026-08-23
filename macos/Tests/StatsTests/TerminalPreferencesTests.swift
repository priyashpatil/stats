import Foundation
import Testing
@testable import Stats

@Suite("Terminal preferences", .serialized)
struct TerminalPreferencesTests {
  @Test("Font size defaults to 15 points")
  func fontSizeDefaultsTo15Points() {
    let defaults = isolatedDefaults()
    defer { defaults.removePersistentDomain(forName: suiteName) }

    #expect(TerminalPreferences(defaults: defaults).fontSize == 15)
  }

  @Test("Saved font size is restored")
  func savedFontSizeIsRestored() {
    let defaults = isolatedDefaults()
    defer { defaults.removePersistentDomain(forName: suiteName) }
    let preferences = TerminalPreferences(defaults: defaults)

    preferences.saveFontSize(18)

    #expect(preferences.fontSize == 18)
  }

  private let suiteName = "StatsTests.TerminalPreferences"

  private func isolatedDefaults() -> UserDefaults {
    let defaults = UserDefaults(suiteName: suiteName)!
    defaults.removePersistentDomain(forName: suiteName)
    return defaults
  }
}
