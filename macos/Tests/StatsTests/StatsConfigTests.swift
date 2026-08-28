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
    let store = try StatsConfigStore(url: fixture.url)
    let clocks = [
      ClockChoice(label: "London", timezone: "Europe/London"),
      ClockChoice(label: "New York", timezone: "America/New_York"),
      ClockChoice(label: "Tokyo", timezone: "Asia/Tokyo"),
      ClockChoice(label: "Sydney", timezone: "Australia/Sydney"),
    ]

    try store.saveClocks(clocks)
    let sections = SectionsConfig(ai: false, ampActivity: false)
    let display = SectionDisplayConfig(
      system: SystemDisplayConfig(cpu: false, network: false),
      ai: AIDisplayConfig(ampCredits: false)
    )
    try store.saveSectionSettings(sections, display: display)
    try store.saveFontSize(18)
    try store.saveShowScrollbar(false)

    let restored = try StatsConfigStore(url: fixture.url)
    #expect(restored.config.clocks.map(\.id) == clocks.map(\.id))
    #expect(restored.config.sections == sections)
    #expect(restored.config.sectionDisplay == display)
    #expect(restored.config.desktop.fontSize == 18)
    #expect(restored.config.desktop.showScrollbar == false)
    let contents = try String(contentsOf: fixture.url, encoding: .utf8)
    #expect(contents.contains("[[clocks]]"))
    #expect(contents.contains("amp_activity = false"))
    #expect(contents.contains("amp_credits = false"))
    #expect(contents.contains("font_size = 18"))
    #expect(contents.contains("show_scrollbar = false"))
  }

  @Test("Version one config is rejected")
  func versionOneConfigIsRejected() throws {
    let fixture = try fixture()
    defer { fixture.cleanup() }
    try writeConfig(
      validConfig().replacingOccurrences(of: "version = 2", with: "version = 1"),
      to: fixture.url
    )

    #expect(throws: (any Error).self) {
      try StatsConfigStore(url: fixture.url)
    }
  }

  @Test("Missing section display table is rejected")
  func missingSectionDisplayTableIsRejected() throws {
    let fixture = try fixture()
    defer { fixture.cleanup() }
    try writeConfig(
      validConfig().replacingOccurrences(
        of: "[section_display.clocks]",
        with: "[removed_clocks]"
      ),
      to: fixture.url
    )

    #expect(throws: (any Error).self) {
      try StatsConfigStore(url: fixture.url)
    }
  }

  @Test("Missing section display option is rejected")
  func missingSectionDisplayOptionIsRejected() throws {
    let fixture = try fixture()
    defer { fixture.cleanup() }
    try writeConfig(
      validConfig().replacingOccurrences(of: "clock_1 = true\n", with: ""),
      to: fixture.url
    )

    #expect(throws: (any Error).self) {
      try StatsConfigStore(url: fixture.url)
    }
  }

  @Test("Enabled section requires one display option")
  func enabledSectionRequiresDisplayOption() throws {
    let fixture = try fixture()
    defer { fixture.cleanup() }
    try writeConfig(configWithEmptySystem(enabled: true), to: fixture.url)

    #expect(throws: (any Error).self) {
      try StatsConfigStore(url: fixture.url)
    }
  }

  @Test("Disabled section may have no display options")
  func disabledSectionAllowsNoDisplayOptions() throws {
    let fixture = try fixture()
    defer { fixture.cleanup() }
    try writeConfig(configWithEmptySystem(enabled: false), to: fixture.url)

    let store = try StatsConfigStore(url: fixture.url)

    #expect(store.config.sections.system == false)
    #expect(store.config.sectionDisplay.system.hasEnabledOption == false)
  }

  @Test("Config created before Claude integration remains compatible")
  func preClaudeConfigRemainsCompatible() throws {
    let fixture = try fixture()
    defer { fixture.cleanup() }
    try writeConfig(
      validConfig()
        .replacingOccurrences(of: "    claude_quota = true\n", with: "")
        .replacingOccurrences(of: "    claude_seconds = 300\n", with: ""),
      to: fixture.url
    )

    let store = try StatsConfigStore(url: fixture.url)

    #expect(store.config.sectionDisplay.ai.claudeQuota == false)
    #expect(store.config.refresh.claudeSeconds == 300)
  }

  @Test("App updates preserve edits made directly to the file")
  func appUpdatesPreserveExternalEdits() throws {
    let fixture = try fixture()
    defer { fixture.cleanup() }
    let store = try StatsConfigStore(url: fixture.url)
    try store.ensureFileExists()
    let contents = try String(contentsOf: fixture.url, encoding: .utf8)
      .replacingOccurrences(of: "amp_seconds = 300", with: "amp_seconds = 600")
    try contents.write(to: fixture.url, atomically: true, encoding: .utf8)

    try store.saveFontSize(20)

    let restored = try StatsConfigStore(url: fixture.url)
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
      try StatsConfigStore(url: fixture.url)
    }
    #expect(try String(contentsOf: fixture.url, encoding: .utf8) == "version = 2\n")
  }

  private func fixture() throws -> Fixture {
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("stats-config-\(UUID().uuidString)", isDirectory: true)
    return Fixture(
      directory: directory,
      url: directory.appendingPathComponent("config.toml")
    )
  }

  private func configWithEmptySystem(enabled: Bool) -> String {
    validConfig()
      .replacingOccurrences(of: "system = true", with: "system = \(enabled)")
      .replacingOccurrences(
        of: """
          [section_display.system]
          heading = true
          cpu = true
          ram = true
          gpu = true
          storage = true
          network = true
          """,
        with: """
          [section_display.system]
          heading = false
          cpu = false
          ram = false
          gpu = false
          storage = false
          network = false
          """
      )
  }

  private func validConfig() -> String {
    """
    version = 2

    [[clocks]]
    label = "Mumbai"
    timezone = "Asia/Kolkata"
    [[clocks]]
    label = "Paris"
    timezone = "Europe/Paris"
    [[clocks]]
    label = "Sydney"
    timezone = "Australia/Sydney"
    [[clocks]]
    label = "Seattle"
    timezone = "America/Los_Angeles"

    [sections]
    clocks = true
    system = true
    ai = true
    amp_activity = true
    codex_activity = true

    [section_display.clocks]
    heading = true
    clock_1 = true
    clock_2 = true
    clock_3 = true
    clock_4 = true

    [section_display.system]
    heading = true
    cpu = true
    ram = true
    gpu = true
    storage = true
    network = true

    [section_display.ai]
    heading = true
    amp_plan = true
    amp_orbs = true
    amp_credits = true
    codex_quota = true
    claude_quota = true

    [section_display.amp_activity]
    heading = true
    calendar = true
    daily_activity = true
    usage_summary = true
    models = true
    sources = true
    sync_alerts = true

    [section_display.codex_activity]
    heading = true
    calendar = true
    overview = true
    daily_activity = true

    [refresh]
    codex_seconds = 60
    amp_seconds = 300
    claude_seconds = 300
    storage_seconds = 300

    [desktop]
    font_size = 15
    show_scrollbar = false
    """
  }

  private func writeConfig(_ contents: String, to url: URL) throws {
    try FileManager.default.createDirectory(
      at: url.deletingLastPathComponent(),
      withIntermediateDirectories: true
    )
    try contents.write(to: url, atomically: true, encoding: .utf8)
  }
}

private struct Fixture {
  let directory: URL
  let url: URL

  func cleanup() {
    try? FileManager.default.removeItem(at: directory)
  }
}
