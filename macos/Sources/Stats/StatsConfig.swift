import Foundation
import TOML

struct StatsConfig: Codable {
  static let currentVersion = 1

  var version: Int
  var clocks: [ClockChoice]
  var sections: SectionsConfig
  var refresh: RefreshConfig
  var desktop: DesktopConfig

  init(
    version: Int = currentVersion,
    clocks: [ClockChoice] = ClockChoice.defaults,
    sections: SectionsConfig = SectionsConfig(),
    refresh: RefreshConfig = RefreshConfig(),
    desktop: DesktopConfig = DesktopConfig()
  ) {
    self.version = version
    self.clocks = clocks
    self.sections = sections
    self.refresh = refresh
    self.desktop = desktop
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    version = try container.decodeIfPresent(Int.self, forKey: .version) ?? Self.currentVersion
    clocks = try container.decodeIfPresent([ClockChoice].self, forKey: .clocks)
      ?? ClockChoice.defaults
    sections = try container.decodeIfPresent(SectionsConfig.self, forKey: .sections)
      ?? SectionsConfig()
    refresh = try container.decodeIfPresent(RefreshConfig.self, forKey: .refresh)
      ?? RefreshConfig()
    desktop = try container.decodeIfPresent(DesktopConfig.self, forKey: .desktop)
      ?? DesktopConfig()
  }
}

struct SectionsConfig: Codable, Equatable {
  var clocks: Bool
  var system: Bool
  var ai: Bool
  var ampActivity: Bool
  var codexActivity: Bool

  init(
    clocks: Bool = true,
    system: Bool = true,
    ai: Bool = true,
    ampActivity: Bool = true,
    codexActivity: Bool = true
  ) {
    self.clocks = clocks
    self.system = system
    self.ai = ai
    self.ampActivity = ampActivity
    self.codexActivity = codexActivity
  }

  enum CodingKeys: String, CodingKey {
    case clocks
    case system
    case ai
    case ampActivity = "amp_activity"
    case codexActivity = "codex_activity"
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    clocks = try container.decodeIfPresent(Bool.self, forKey: .clocks) ?? true
    system = try container.decodeIfPresent(Bool.self, forKey: .system) ?? true
    ai = try container.decodeIfPresent(Bool.self, forKey: .ai) ?? true
    ampActivity = try container.decodeIfPresent(Bool.self, forKey: .ampActivity) ?? true
    codexActivity = try container.decodeIfPresent(Bool.self, forKey: .codexActivity) ?? true
  }
}

struct RefreshConfig: Codable {
  var codexSeconds: Int
  var ampSeconds: Int
  var storageSeconds: Int

  init(codexSeconds: Int = 60, ampSeconds: Int = 300, storageSeconds: Int = 300) {
    self.codexSeconds = codexSeconds
    self.ampSeconds = ampSeconds
    self.storageSeconds = storageSeconds
  }

  enum CodingKeys: String, CodingKey {
    case codexSeconds = "codex_seconds"
    case ampSeconds = "amp_seconds"
    case storageSeconds = "storage_seconds"
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    codexSeconds = try container.decodeIfPresent(Int.self, forKey: .codexSeconds) ?? 60
    ampSeconds = try container.decodeIfPresent(Int.self, forKey: .ampSeconds) ?? 300
    storageSeconds = try container.decodeIfPresent(Int.self, forKey: .storageSeconds) ?? 300
  }
}

struct DesktopConfig: Codable {
  var fontSize: Int
  var showScrollbar: Bool

  init(
    fontSize: Int = StatsConfigStore.defaultFontSize,
    showScrollbar: Bool = false
  ) {
    self.fontSize = fontSize
    self.showScrollbar = showScrollbar
  }

  enum CodingKeys: String, CodingKey {
    case fontSize = "font_size"
    case showScrollbar = "show_scrollbar"
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    fontSize = try container.decodeIfPresent(Int.self, forKey: .fontSize)
      ?? StatsConfigStore.defaultFontSize
    showScrollbar = try container.decodeIfPresent(Bool.self, forKey: .showScrollbar) ?? false
  }
}

final class StatsConfigStore {
  static let availableFontSizes = Array(10...24)
  static let defaultFontSize = 15

  let url: URL
  private(set) var config: StatsConfig

  init(
    url: URL = StatsConfigStore.defaultURL(),
    defaults: UserDefaults = .standard
  ) throws {
    self.url = url
    if FileManager.default.fileExists(atPath: url.path) {
      config = try Self.load(url)
    } else {
      config = Self.migratedConfig(from: defaults)
      if Self.hasLegacyConfig(in: defaults) {
        try write(config)
      }
    }
  }

  static func defaultURL(
    environment: [String: String] = ProcessInfo.processInfo.environment,
    homeDirectory: URL = FileManager.default.homeDirectoryForCurrentUser
  ) -> URL {
    if let configured = environment["XDG_CONFIG_HOME"], !configured.isEmpty {
      if (configured as NSString).isAbsolutePath {
        return URL(fileURLWithPath: configured, isDirectory: true)
          .appendingPathComponent("stats/config.toml")
      }
    }
    return homeDirectory.appendingPathComponent(".config/stats/config.toml")
  }

  func saveClocks(_ clocks: [ClockChoice]) throws {
    var updated = try configForUpdate()
    updated.clocks = clocks
    try Self.validate(updated)
    try write(updated)
    config = updated
  }

  func saveSections(_ sections: SectionsConfig) throws {
    var updated = try configForUpdate()
    updated.sections = sections
    try Self.validate(updated)
    try write(updated)
    config = updated
  }

  func saveFontSize(_ fontSize: Int) throws {
    var updated = try configForUpdate()
    updated.desktop.fontSize = fontSize
    try Self.validate(updated)
    try write(updated)
    config = updated
  }

  func saveShowScrollbar(_ showScrollbar: Bool) throws {
    var updated = try configForUpdate()
    updated.desktop.showScrollbar = showScrollbar
    try Self.validate(updated)
    try write(updated)
    config = updated
  }

  func ensureFileExists() throws {
    if !FileManager.default.fileExists(atPath: url.path) {
      try write(config)
    }
  }

  private func configForUpdate() throws -> StatsConfig {
    guard FileManager.default.fileExists(atPath: url.path) else { return config }
    return try Self.load(url)
  }

  private func write(_ config: StatsConfig) throws {
    let directory = url.deletingLastPathComponent()
    try FileManager.default.createDirectory(
      at: directory,
      withIntermediateDirectories: true,
      attributes: [.posixPermissions: 0o700]
    )
    try FileManager.default.setAttributes(
      [.posixPermissions: 0o700],
      ofItemAtPath: directory.path
    )
    let data = try TOMLEncoder().encode(config)
    try data.write(to: url, options: .atomic)
    try FileManager.default.setAttributes(
      [.posixPermissions: 0o600],
      ofItemAtPath: url.path
    )
  }

  private static func decode(_ url: URL) throws -> StatsConfig {
    let data = try Data(contentsOf: url)
    return try TOMLDecoder().decode(StatsConfig.self, from: data)
  }

  private static func load(_ url: URL) throws -> StatsConfig {
    do {
      let config = try decode(url)
      try validate(config)
      return config
    } catch {
      throw ConfigError.invalid("Invalid config at \(url.path): \(error.localizedDescription)")
    }
  }

  private static func validate(_ config: StatsConfig) throws {
    guard config.version == StatsConfig.currentVersion else {
      throw ConfigError.invalid("unsupported version \(config.version)")
    }
    guard config.clocks.count == 4 else {
      throw ConfigError.invalid("clocks must contain exactly 4 entries")
    }
    for clock in config.clocks {
      guard !clock.label.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
        throw ConfigError.invalid("clock labels cannot be empty")
      }
      guard TimeZone(identifier: clock.timezone) != nil else {
        throw ConfigError.invalid("unknown clock timezone: \(clock.timezone)")
      }
    }
    guard config.refresh.codexSeconds >= 5 else {
      throw ConfigError.invalid("refresh.codex_seconds must be at least 5")
    }
    guard config.refresh.ampSeconds >= 60 else {
      throw ConfigError.invalid("refresh.amp_seconds must be at least 60")
    }
    guard config.refresh.storageSeconds >= 60 else {
      throw ConfigError.invalid("refresh.storage_seconds must be at least 60")
    }
    guard availableFontSizes.contains(config.desktop.fontSize) else {
      throw ConfigError.invalid("desktop.font_size must be between 10 and 24")
    }
  }

  private static func hasLegacyConfig(in defaults: UserDefaults) -> Bool {
    defaults.object(forKey: "selectedClockCities") != nil
      || defaults.object(forKey: "selectedClockTimezones") != nil
      || defaults.object(forKey: "terminalFontSize") != nil
  }

  private static func migratedConfig(from defaults: UserDefaults) -> StatsConfig {
    let storedIDs = defaults.stringArray(forKey: "selectedClockCities")
    var clocks = storedIDs?.compactMap { id in
      ClockChoice.all.first(where: { $0.id == id })
    } ?? []
    if storedIDs == nil {
      clocks = defaults.stringArray(forKey: "selectedClockTimezones")?
        .compactMap(ClockChoice.choice(forLegacyTimezone:)) ?? []
    }
    for choice in ClockChoice.defaults where clocks.count < 4 {
      if !clocks.contains(where: { $0.id == choice.id }) {
        clocks.append(choice)
      }
    }

    let storedFontSize = defaults.integer(forKey: "terminalFontSize")
    let fontSize = availableFontSizes.contains(storedFontSize)
      ? storedFontSize : defaultFontSize
    return StatsConfig(
      clocks: Array(clocks.prefix(4)),
      desktop: DesktopConfig(fontSize: fontSize)
    )
  }
}

enum ConfigError: LocalizedError {
  case invalid(String)

  var errorDescription: String? {
    switch self {
    case .invalid(let message):
      return message
    }
  }
}
