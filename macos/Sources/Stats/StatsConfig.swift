import Foundation
import TOML

struct StatsConfig: Codable {
  static let currentVersion = 2

  var version: Int
  var clocks: [ClockChoice]
  var sections: SectionsConfig
  var sectionDisplay: SectionDisplayConfig
  var refresh: RefreshConfig
  var desktop: DesktopConfig

  init(
    version: Int = currentVersion,
    clocks: [ClockChoice] = ClockChoice.defaults,
    sections: SectionsConfig = SectionsConfig(),
    sectionDisplay: SectionDisplayConfig = SectionDisplayConfig(),
    refresh: RefreshConfig = RefreshConfig(),
    desktop: DesktopConfig = DesktopConfig()
  ) {
    self.version = version
    self.clocks = clocks
    self.sections = sections
    self.sectionDisplay = sectionDisplay
    self.refresh = refresh
    self.desktop = desktop
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    version = try container.decode(Int.self, forKey: .version)
    clocks = try container.decode([ClockChoice].self, forKey: .clocks)
    sections = try container.decode(SectionsConfig.self, forKey: .sections)
    sectionDisplay = try container.decode(SectionDisplayConfig.self, forKey: .sectionDisplay)
    refresh = try container.decode(RefreshConfig.self, forKey: .refresh)
    desktop = try container.decode(DesktopConfig.self, forKey: .desktop)
  }

  enum CodingKeys: String, CodingKey {
    case version
    case clocks
    case sections
    case sectionDisplay = "section_display"
    case refresh
    case desktop
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
    clocks = try container.decode(Bool.self, forKey: .clocks)
    system = try container.decode(Bool.self, forKey: .system)
    ai = try container.decode(Bool.self, forKey: .ai)
    ampActivity = try container.decode(Bool.self, forKey: .ampActivity)
    codexActivity = try container.decode(Bool.self, forKey: .codexActivity)
  }
}

struct SectionDisplayConfig: Codable, Equatable {
  var clocks: ClocksDisplayConfig
  var system: SystemDisplayConfig
  var ai: AIDisplayConfig
  var ampActivity: AmpActivityDisplayConfig
  var codexActivity: CodexActivityDisplayConfig

  init(
    clocks: ClocksDisplayConfig = ClocksDisplayConfig(),
    system: SystemDisplayConfig = SystemDisplayConfig(),
    ai: AIDisplayConfig = AIDisplayConfig(),
    ampActivity: AmpActivityDisplayConfig = AmpActivityDisplayConfig(),
    codexActivity: CodexActivityDisplayConfig = CodexActivityDisplayConfig()
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
    clocks = try container.decode(ClocksDisplayConfig.self, forKey: .clocks)
    system = try container.decode(SystemDisplayConfig.self, forKey: .system)
    ai = try container.decode(AIDisplayConfig.self, forKey: .ai)
    ampActivity = try container.decode(AmpActivityDisplayConfig.self, forKey: .ampActivity)
    codexActivity = try container.decode(CodexActivityDisplayConfig.self, forKey: .codexActivity)
  }
}

struct ClocksDisplayConfig: Codable, Equatable {
  var heading = true
  var clock1 = true
  var clock2 = true
  var clock3 = true
  var clock4 = true

  enum CodingKeys: String, CodingKey {
    case heading
    case clock1 = "clock_1"
    case clock2 = "clock_2"
    case clock3 = "clock_3"
    case clock4 = "clock_4"
  }

  init(
    heading: Bool = true,
    clock1: Bool = true,
    clock2: Bool = true,
    clock3: Bool = true,
    clock4: Bool = true
  ) {
    self.heading = heading
    self.clock1 = clock1
    self.clock2 = clock2
    self.clock3 = clock3
    self.clock4 = clock4
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    heading = try container.decode(Bool.self, forKey: .heading)
    clock1 = try container.decode(Bool.self, forKey: .clock1)
    clock2 = try container.decode(Bool.self, forKey: .clock2)
    clock3 = try container.decode(Bool.self, forKey: .clock3)
    clock4 = try container.decode(Bool.self, forKey: .clock4)
  }

  var hasEnabledOption: Bool { heading || clock1 || clock2 || clock3 || clock4 }
}

struct SystemDisplayConfig: Codable, Equatable {
  var heading = true
  var cpu = true
  var ram = true
  var gpu = true
  var storage = true
  var network = true

  init(
    heading: Bool = true,
    cpu: Bool = true,
    ram: Bool = true,
    gpu: Bool = true,
    storage: Bool = true,
    network: Bool = true
  ) {
    self.heading = heading
    self.cpu = cpu
    self.ram = ram
    self.gpu = gpu
    self.storage = storage
    self.network = network
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    heading = try container.decode(Bool.self, forKey: .heading)
    cpu = try container.decode(Bool.self, forKey: .cpu)
    ram = try container.decode(Bool.self, forKey: .ram)
    gpu = try container.decode(Bool.self, forKey: .gpu)
    storage = try container.decode(Bool.self, forKey: .storage)
    network = try container.decode(Bool.self, forKey: .network)
  }

  var hasEnabledOption: Bool { heading || cpu || ram || gpu || storage || network }
}

struct AIDisplayConfig: Codable, Equatable {
  var heading = true
  var ampPlan = true
  var ampOrbs = true
  var ampCredits = true
  var codexQuota = true

  enum CodingKeys: String, CodingKey {
    case heading
    case ampPlan = "amp_plan"
    case ampOrbs = "amp_orbs"
    case ampCredits = "amp_credits"
    case codexQuota = "codex_quota"
  }

  init(
    heading: Bool = true,
    ampPlan: Bool = true,
    ampOrbs: Bool = true,
    ampCredits: Bool = true,
    codexQuota: Bool = true
  ) {
    self.heading = heading
    self.ampPlan = ampPlan
    self.ampOrbs = ampOrbs
    self.ampCredits = ampCredits
    self.codexQuota = codexQuota
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    heading = try container.decode(Bool.self, forKey: .heading)
    ampPlan = try container.decode(Bool.self, forKey: .ampPlan)
    ampOrbs = try container.decode(Bool.self, forKey: .ampOrbs)
    ampCredits = try container.decode(Bool.self, forKey: .ampCredits)
    codexQuota = try container.decode(Bool.self, forKey: .codexQuota)
  }

  var hasEnabledOption: Bool { heading || ampPlan || ampOrbs || ampCredits || codexQuota }
}

struct AmpActivityDisplayConfig: Codable, Equatable {
  var heading = true
  var calendar = true
  var dailyActivity = true
  var usageSummary = true
  var models = true
  var sources = true
  var syncAlerts = true

  enum CodingKeys: String, CodingKey {
    case heading
    case calendar
    case dailyActivity = "daily_activity"
    case usageSummary = "usage_summary"
    case models
    case sources
    case syncAlerts = "sync_alerts"
  }

  init(
    heading: Bool = true,
    calendar: Bool = true,
    dailyActivity: Bool = true,
    usageSummary: Bool = true,
    models: Bool = true,
    sources: Bool = true,
    syncAlerts: Bool = true
  ) {
    self.heading = heading
    self.calendar = calendar
    self.dailyActivity = dailyActivity
    self.usageSummary = usageSummary
    self.models = models
    self.sources = sources
    self.syncAlerts = syncAlerts
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    heading = try container.decode(Bool.self, forKey: .heading)
    calendar = try container.decode(Bool.self, forKey: .calendar)
    dailyActivity = try container.decode(Bool.self, forKey: .dailyActivity)
    usageSummary = try container.decode(Bool.self, forKey: .usageSummary)
    models = try container.decode(Bool.self, forKey: .models)
    sources = try container.decode(Bool.self, forKey: .sources)
    syncAlerts = try container.decode(Bool.self, forKey: .syncAlerts)
  }

  var hasEnabledOption: Bool {
    heading || calendar || dailyActivity || usageSummary || models || sources || syncAlerts
  }
}

struct CodexActivityDisplayConfig: Codable, Equatable {
  var heading = true
  var calendar = true
  var overview = true
  var dailyActivity = true

  enum CodingKeys: String, CodingKey {
    case heading
    case calendar
    case overview
    case dailyActivity = "daily_activity"
  }

  init(
    heading: Bool = true,
    calendar: Bool = true,
    overview: Bool = true,
    dailyActivity: Bool = true
  ) {
    self.heading = heading
    self.calendar = calendar
    self.overview = overview
    self.dailyActivity = dailyActivity
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    heading = try container.decode(Bool.self, forKey: .heading)
    calendar = try container.decode(Bool.self, forKey: .calendar)
    overview = try container.decode(Bool.self, forKey: .overview)
    dailyActivity = try container.decode(Bool.self, forKey: .dailyActivity)
  }

  var hasEnabledOption: Bool { heading || calendar || overview || dailyActivity }
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
    codexSeconds = try container.decode(Int.self, forKey: .codexSeconds)
    ampSeconds = try container.decode(Int.self, forKey: .ampSeconds)
    storageSeconds = try container.decode(Int.self, forKey: .storageSeconds)
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
    fontSize = try container.decode(Int.self, forKey: .fontSize)
    showScrollbar = try container.decode(Bool.self, forKey: .showScrollbar)
  }
}

final class StatsConfigStore {
  static let availableFontSizes = Array(10...24)
  static let defaultFontSize = 15

  let url: URL
  private(set) var config: StatsConfig

  init(url: URL = StatsConfigStore.defaultURL()) throws {
    self.url = url
    if FileManager.default.fileExists(atPath: url.path) {
      config = try Self.load(url)
    } else {
      config = StatsConfig()
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

  func saveSectionSettings(_ sections: SectionsConfig, display: SectionDisplayConfig) throws {
    var updated = try configForUpdate()
    updated.sections = sections
    updated.sectionDisplay = display
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
    let requirements: [(Bool, Bool, String)] = [
      (config.sections.clocks, config.sectionDisplay.clocks.hasEnabledOption, "clocks"),
      (config.sections.system, config.sectionDisplay.system.hasEnabledOption, "system"),
      (config.sections.ai, config.sectionDisplay.ai.hasEnabledOption, "ai"),
      (
        config.sections.ampActivity,
        config.sectionDisplay.ampActivity.hasEnabledOption,
        "amp_activity"
      ),
      (
        config.sections.codexActivity,
        config.sectionDisplay.codexActivity.hasEnabledOption,
        "codex_activity"
      ),
    ]
    for (enabled, hasEnabledOption, section) in requirements where enabled && !hasEnabledOption {
      throw ConfigError.invalid(
        "sections.\(section) requires at least one section_display.\(section) option"
      )
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
