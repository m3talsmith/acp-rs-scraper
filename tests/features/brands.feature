Feature: Brands
  The logo, title, and motto travel with a theme. Built-in brands still ship
  with the program. A file in ~/.acp/brands overrides a built-in of the same
  name. --brand selects one for this run. Saving a theme writes that branding
  into the theme file.

  Background:
    Given a mock Red Star site

  Scenario: Built-in brands are listed when the folder is empty
    When I run "--list-brands"
    Then it succeeds
    And stdout contains "● default"
    And stdout contains "ACP Red Star"
    And stdout contains "plain"
    And stdout contains "Red Star"
    And stdout contains "(no motto)"
    And stdout contains "Research starts here."

  Scenario: --brand plain changes help and version and does not save the choice
    When I run "--brand plain -h"
    Then it succeeds
    And stdout contains "Red Star: a local client"
    And stdout does not contain "ACP Red Star"
    When I run "--brand plain --help"
    Then it succeeds
    And stdout contains "Red Star. Run with no flags"
    And stdout does not contain "ACP Red Star"
    When I run "--brand plain --version"
    Then it succeeds
    And stdout contains "acp-red-star"
    And stdout contains "Red Star"
    And stdout does not contain "ACP Red Star"
    And the file "$HOME/.acp/preferences.toml" does not contain "brand"
    When I run "--help"
    Then stdout contains "ACP Red Star"

  Scenario: The theme editor preview sits beside the editor and follows branding
    When I edit the theme preview branding to title "North Star", motto "hold the line", and logo text "NS"
    Then the theme preview contains "Theme editor"
    And the theme preview contains "Branding"
    And the theme preview contains "Preview (sample posts)"
    And the theme preview contains "North Star"
    And the theme preview contains "hold the line"
    And the theme preview contains "NS"
    And the theme preview contains "#112233"

  Scenario: The theme editor preview uses the selected brand and the sample nicks
    When I open the theme editor preview on the brand "plain"
    Then the theme preview contains "Red Star"
    And the theme preview contains "★"
    And the theme preview contains "@sample"
    And the theme preview contains "@reader"
    And the theme preview does not contain "ACP Red Star"
    And the theme preview does not contain "Research starts here."
    And the theme preview does not contain "comrade"

  Scenario: --brand plain still opens the terminal UI
    When I run "--brand plain"
    Then it fails
    And stderr contains "the terminal UI needs a terminal"

  Scenario: A user brand overrides a built-in and a motto can be empty
    Given a local file "$HOME/.acp/brands/plain.toml" containing "version = 1\ntitle = 'North Star'\nmotto = ''\nword = ''\nlogo = 'text'\nlogo_text = '*'\n"
    When I run "--list-brands --brand plain"
    Then it succeeds
    And stdout contains "● plain"
    And stdout contains "North Star"
    And stdout contains "(no motto)"
    And stdout does not contain "● default"

  Scenario: A logo path that leaves the brands folder is refused and the file stays
    Given a local file "$HOME/.acp/brands/bad.toml" containing "version = 1\ntitle = 'X'\nlogo = 'file'\nlogo_file = '../secret.png'\n"
    When I run "--brand bad --version"
    Then it fails
    And stderr contains "stay inside"
    And the file "$HOME/.acp/brands/bad.toml" contains "../secret.png"
    When I run "--brand ../x --version"
    Then it fails
    And stderr contains "invalid brand name"

  Scenario: A broken brand file is reported and never overwritten
    Given a local file "$HOME/.acp/brands/broken.toml" containing "version = [oops"
    When I run "--list-brands"
    Then it succeeds
    And stdout contains "BROKEN"
    And the file "$HOME/.acp/brands/broken.toml" contains "version = [oops"
    When I run "--brand broken --version"
    Then it fails
    And stderr contains "broken.toml"
    And the file "$HOME/.acp/brands/broken.toml" contains "version = [oops"

  Scenario: The Red Star logo is the star-blade halberd
    When I open the theme editor preview on the brand "default"
    Then the theme preview contains "halberd"
    And the theme preview contains "RED STAR"
    And the theme preview contains "Research starts here."
    When I open the theme editor preview on the brand "plain"
    Then the theme preview contains "★"
    And the theme preview does not contain "RED STAR"
    And the theme preview does not contain "halberd"

  Scenario: The persisted brand is used when --brand is omitted
    Given a local file "$HOME/.acp/preferences.toml" containing "[ui]\nbrand = 'plain'\n"
    When I run "--version"
    Then it succeeds
    And stdout contains "Red Star"
    And stdout does not contain "ACP Red Star"
    When I run "--brand default --version"
    Then stdout contains "ACP Red Star"
