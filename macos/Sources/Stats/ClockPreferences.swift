import Foundation

struct ClockPreferences {
  private let defaults: UserDefaults
  private let clocksKey = "selectedClockCities"
  private let legacyClocksKey = "selectedClockTimezones"

  init(defaults: UserDefaults = .standard) {
    self.defaults = defaults
  }

  var selectedChoices: [ClockChoice] {
    let storedIDs = defaults.stringArray(forKey: clocksKey)
    var choices =
      storedIDs?.compactMap { id in
        ClockChoice.all.first(where: { $0.id == id })
      } ?? []
    if storedIDs == nil {
      choices =
        defaults.stringArray(forKey: legacyClocksKey)?
        .compactMap(ClockChoice.choice(forLegacyTimezone:)) ?? []
    }
    for choice in ClockChoice.defaults where choices.count < 4 {
      if !choices.contains(where: { $0.id == choice.id }) {
        choices.append(choice)
      }
    }
    return Array(choices.prefix(4))
  }

  func save(_ choices: [ClockChoice]) {
    defaults.set(choices.map(\.id), forKey: clocksKey)
  }
}
