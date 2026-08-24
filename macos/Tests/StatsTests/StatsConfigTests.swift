import Foundation
import Testing
@testable import Stats

@Suite("Stats config", .serialized)
struct StatsConfigTests {
  @Test("Default path uses XDG_CONFIG_HOME")
  func defaultPathUsesXDGConfigHome() {
    let url = StatsConfigStore.defaultURL(
      environment: ["XDG_CONFIG_HOME": "/tmp/custom-config"],
      homeDirectory: URL(fileURLWithPath: "/Users/example", isDirectory: true)
    )

    #expect(url.path == "/tmp/custom-config/stats/config.toml")
  }

  @Test("Default path falls back to dot config")
  func defaultPathFallsBackToDotConfig() {
    let url = StatsConfigStore.defaultURL(
      environment: [:],
      homeDirectory: URL(fileURLWithPath: "/Users/example", isDirectory: true)
    )

    #expect(url.path == "/Users/example/.config/stats/config.toml")
  }

  @Test("Saved settings round trip through TOML")
  func settingsRoundTrip() throws {
    let fixture = try fixture()
    defer { fixture.cleanup() }
    let store = try StatsConfigStore(url: fixture.url, defaults: fixture.defaults)
    let clocks = [
      ClockChoice(label: "London", timezone: "Europe/London"),
      ClockChoice(label: "New York", timezone: "America/New_York"),
      ClockChoice(label: "Tokyo", timezone: "Asia/Tokyo"),
      ClockChoice(label: "Sydney", timezone: "Australia/Sydney"),
    ]

    try store.saveClocks(clocks)
    try store.saveFontSize(18)

    let restored = try StatsConfigStore(url: fixture.url, defaults: fixture.defaults)
    #expect(restored.config.clocks.map(\.id) == clocks.map(\.id))
    #expect(restored.config.desktop.fontSize == 18)
    let contents = try String(contentsOf: fixture.url, encoding: .utf8)
    #expect(contents.contains("[[clocks]]"))
    #expect(contents.contains("font_size = 18"))
  }

  @Test("Legacy defaults migrate once when no config exists")
  func legacyDefaultsMigrate() throws {
    let fixture = try fixture()
    defer { fixture.cleanup() }
    fixture.defaults.set(
      [ClockChoice.defaults[2].id, ClockChoice.defaults[0].id],
      forKey: "selectedClockCities"
    )
    fixture.defaults.set(19, forKey: "terminalFontSize")

    let store = try StatsConfigStore(url: fixture.url, defaults: fixture.defaults)

    #expect(store.config.clocks.prefix(2).map(\.id) == [
      ClockChoice.defaults[2].id,
      ClockChoice.defaults[0].id,
    ])
    #expect(store.config.desktop.fontSize == 19)
    #expect(FileManager.default.fileExists(atPath: fixture.url.path))
  }

  @Test("App updates preserve edits made directly to the file")
  func appUpdatesPreserveExternalEdits() throws {
    let fixture = try fixture()
    defer { fixture.cleanup() }
    let store = try StatsConfigStore(url: fixture.url, defaults: fixture.defaults)
    try store.ensureFileExists()
    let contents = try String(contentsOf: fixture.url, encoding: .utf8)
      .replacingOccurrences(of: "amp_seconds = 300", with: "amp_seconds = 600")
    try contents.write(to: fixture.url, atomically: true, encoding: .utf8)

    try store.saveFontSize(20)

    let restored = try StatsConfigStore(url: fixture.url, defaults: fixture.defaults)
    #expect(restored.config.refresh.ampSeconds == 600)
    #expect(restored.config.desktop.fontSize == 20)
  }

  @Test("Invalid config is reported instead of overwritten")
  func invalidConfigIsReported() throws {
    let fixture = try fixture()
    defer { fixture.cleanup() }
    try FileManager.default.createDirectory(
      at: fixture.url.deletingLastPathComponent(),
      withIntermediateDirectories: true
    )
    try "version = 2\n".write(to: fixture.url, atomically: true, encoding: .utf8)

    #expect(throws: (any Error).self) {
      try StatsConfigStore(url: fixture.url, defaults: fixture.defaults)
    }
    #expect(try String(contentsOf: fixture.url, encoding: .utf8) == "version = 2\n")
  }

  private func fixture() throws -> Fixture {
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("stats-config-\(UUID().uuidString)", isDirectory: true)
    let suite = "StatsTests.StatsConfig.\(UUID().uuidString)"
    let defaults = try #require(UserDefaults(suiteName: suite))
    defaults.removePersistentDomain(forName: suite)
    return Fixture(
      directory: directory,
      url: directory.appendingPathComponent("config.toml"),
      defaults: defaults,
      suite: suite
    )
  }
}

private struct Fixture {
  let directory: URL
  let url: URL
  let defaults: UserDefaults
  let suite: String

  func cleanup() {
    defaults.removePersistentDomain(forName: suite)
    try? FileManager.default.removeItem(at: directory)
  }
}
