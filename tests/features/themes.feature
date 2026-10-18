Feature: Theme files
  Every theme is a TOML file in ~/.acp/themes. The first run copies the
  shipped themes there; --list-themes shows them and --theme picks one for a
  run, strictly.

  Background:
    Given a mock Red Star site

  Scenario: The first run installs the shipped default and lists it
    When I run "--list-themes"
    Then it succeeds
    And stdout contains "Themes in"
    And stdout contains "● default"
    And stdout contains "ACP red on your terminal's own colours"
    And the file "$HOME/.acp/themes/default.toml" contains "#dc2626"

  Scenario: --theme marks the theme for this run only
    Given a local file "$HOME/.acp/themes/night.toml" containing "version = 1\nname = 'night'\ndescription = 'Night'\n[dark.palette]\naccent = '#00ff00'\n"
    When I run "--list-themes --theme night"
    Then it succeeds
    And stdout contains "● night                Night"
    And stdout does not contain "● default"
    And the file "$HOME/.acp/preferences.toml" does not contain "night"

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
    And the file "$HOME/.acp/preferences.toml" contains "#112233"
    And the file "$HOME/.acp/preferences.toml" contains "custom"
    When I run "--list-themes"
    Then stdout contains "● custom"
    And the file "$HOME/.acp/themes/custom-2.toml" does not exist
