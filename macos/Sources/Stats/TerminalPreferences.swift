import Foundation

struct TerminalPreferences {
  static let availableFontSizes = Array(10...24)
  static let defaultFontSize = 15

  private let defaults: UserDefaults
  private let fontSizeKey = "terminalFontSize"

  init(defaults: UserDefaults = .standard) {
    self.defaults = defaults
  }

  var fontSize: Int {
    let storedSize = defaults.integer(forKey: fontSizeKey)
    return Self.availableFontSizes.contains(storedSize) ? storedSize : Self.defaultFontSize
  }

  func saveFontSize(_ fontSize: Int) {
    guard Self.availableFontSizes.contains(fontSize) else { return }
    defaults.set(fontSize, forKey: fontSizeKey)
  }
}
