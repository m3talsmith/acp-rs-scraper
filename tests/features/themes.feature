Feature: Theme files
  Every theme is a TOML file in ~/.acp/themes, the only place themes are read from.
  The first run with an empty folder downloads default.toml from the repo (here: the
  mock server); --theme-init fetches on request; --list-themes shows them and
  --theme picks one for a run, strictly.

  Background:
    Given a mock Red Star site

  Scenario: The first run downloads the default theme and lists it
    When I run "--list-themes"
    Then it succeeds
    And stderr contains "downloaded the default theme"
    And stdout contains "Themes in"
    And stdout contains "● default"
    And stdout contains "ACP red on your terminal's own colours"
    And stdout does not contain "soviet"
    And the file "$HOME/.acp/themes/default.toml" contains "#dc2626"
    And the theme server was asked 1 times
    And no cookies were sent for themes
    When I run "--list-themes"
    Then it succeeds
    And the theme server was asked 1 times

  Scenario: Offline, the terminal's colours stay and the notice says how to fix it
    Given the theme server answers "offline"
    When I run "--list-themes"
    Then it succeeds
    And stderr contains "could not download the default theme"
    And stderr contains "--theme-init"
    And stdout contains "(selected theme default is missing"
    And the file "$HOME/.acp/themes/default.toml" does not exist

  Scenario: A server that doesn't answer in time is given up on
    Given the theme server answers "slow"
    When I run "--list-themes"
    Then it succeeds
    And stderr contains "timed out"
    And the file "$HOME/.acp/themes/default.toml" does not exist

  Scenario: Oversized, invalid and missing downloads are refused
    Given the theme server answers "big"
    When I run "--list-themes"
    Then stderr contains "64 KiB"
    And the file "$HOME/.acp/themes/default.toml" does not exist
    Given the theme server answers "bad"
    When I run "--theme-init"
    Then it fails
    And stderr contains "not a valid theme"
    And the file "$HOME/.acp/themes/default.toml" does not exist
    Given the theme server answers "missing"
    When I run "--theme-init"
    Then it fails
    And stderr contains "404"

  Scenario: --theme-init never replaces an existing file
    Given a local file "$HOME/.acp/themes/default.toml" containing "version = 1\nname = 'default'\ndescription = 'Mine'\n"
    When I run "--theme-init"
    Then it succeeds
    And stdout contains "it was kept"
    And the file "$HOME/.acp/themes/default.toml" contains "Mine"
    And the theme server was asked 0 times

  Scenario: --theme-init fetches any shipped theme, or all of them
    When I run "--theme-init soviet"
    Then it succeeds
    And stdout contains "downloaded the soviet theme"
    And the file "$HOME/.acp/themes/soviet.toml" exists
    When I run "--theme-init all"
    Then it succeeds
    And stdout contains "downloaded the plattenbau theme"
    And stdout contains "soviet.toml already exists"
    When I run "--list-themes"
    Then stdout contains "chinese-poster"
    And stdout contains "● default"
    When I run "--theme-init ../x"
    Then it fails
    And stderr contains "invalid theme name"

  Scenario: The automatic download can be turned off with ACP_THEME_DOWNLOAD=off
    Given the environment has "ACP_THEME_DOWNLOAD" set to "off"
    When I run "--list-themes"
    Then it succeeds
    And stderr contains "automatic theme download is off"
    And the theme server was asked 0 times
    And the file "$HOME/.acp/themes/default.toml" does not exist
    When I run "--theme-init"
    Then it succeeds
    And the file "$HOME/.acp/themes/default.toml" exists

  Scenario: The automatic download can be turned off with [ui] theme_download = false
    Given a local file "$HOME/.acp/preferences.toml" containing "[ui]\ntheme_download = false\n"
    When I run "--list-themes"
    Then it succeeds
    And stderr contains "automatic theme download is off"
    And the theme server was asked 0 times

  Scenario: --theme marks the theme for this run only
    Given a local file "$HOME/.acp/themes/night.toml" containing "version = 1\nname = 'night'\ndescription = 'Night'\n[dark.palette]\naccent = '#00ff00'\n"
    When I run "--list-themes --theme night"
    Then it succeeds
    And stdout contains "● night                Night"
    And stdout does not contain "● default"
    And the file "$HOME/.acp/preferences.toml" does not contain "night"
    And the theme server was asked 0 times

  Scenario: --theme refuses unknown and path-escaping names
    When I run "--theme nope --search x"
    Then it fails
    And stderr contains "no theme nope"
    When I run "--theme ../x --search x"
    Then it fails
    And stderr contains "invalid theme name"

  Scenario: A broken theme file is reported and never overwritten
    Given a local file "$HOME/.acp/themes/broken.toml" containing "version = [oops"
    When I run "--list-themes"
    Then it succeeds
    And stdout contains "BROKEN"
    And the file "$HOME/.acp/themes/broken.toml" contains "version = [oops"
    When I run "--theme broken --search x"
    Then it fails
    And stderr contains "broken.toml"

  Scenario: Old [theme] colours become the "custom" theme
    Given a local file "$HOME/.acp/preferences.toml" containing "[theme]\naccent = '#112233'\n"
    When I run "--list-themes"
    Then it succeeds
    And stdout contains "● custom"
    And the file "$HOME/.acp/themes/custom.toml" contains "#112233"
    And the file "$HOME/.acp/themes/custom.toml" contains "#9ca3af"
    And the file "$HOME/.acp/preferences.toml" contains "#112233"
    And the file "$HOME/.acp/preferences.toml" contains "custom"
    When I run "--list-themes"
    Then stdout contains "● custom"
    And the file "$HOME/.acp/themes/custom-2.toml" does not exist
