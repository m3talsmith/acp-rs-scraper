Feature: Account profile and site settings
  A profile card and forum settings can be read. Editing a profile, saving
  settings, and reporting a post or a user print the form and send nothing.

  Background:
    Given a mock Red Star site

  Scenario: A profile card is readable
    When I run "--show-profile alice"
    Then it succeeds
    And stdout contains "profile	alice	ALICE Example	alice	organizing in the shop"
    And stdout does not contain "csrf"
    And stdout does not contain "mock-session"
    And profile "alice" shows "ALICE Example"

  Scenario: Site settings are readable
    When I run "--site-settings"
    Then it succeeds
    And stdout contains "site-settings	time_zone	America/Chicago"
    And stdout contains "site-settings	theme	dark"
    And stdout contains "site-settings	show_sustainer	true"
    And stdout does not contain "csrf"
    And site setting "theme" is "dark"

  Scenario: An edit profile dry-run opens no connection
    When I run "--edit-profile --display-name Ada --username ada --bio hello --dry-run"
    Then it succeeds
    And stdout contains "dry-run: no connection opened"
    And stdout contains "form[display_name]=Ada"
    And stdout contains "form[username]=ada"
    And stdout contains "form[forum_profile][bio]=hello"
    And stdout contains "form=edit-profile-form"
    And stdout contains "DevTools open"
    And the site received 0 account writes

  Scenario: Cancelling a profile edit sends nothing
    When I run "--edit-profile --display-name Ada"
    Then it succeeds
    And stdout contains "nothing sent"
    And the site received 0 account writes

  Scenario: A confirmed profile edit is not sent
    When I run "--edit-profile --display-name Ada --yes"
    Then it fails
    And stderr contains "are not sent"
    And stderr contains "form[display_name]=Ada"
    And the site received 0 account writes

  Scenario: A settings dry-run opens no connection
    When I run "--save-settings --time-zone America/Chicago --site-theme dark --show-sustainer true --dry-run"
    Then it succeeds
    And stdout contains "form[time_zone_id]=America/Chicago"
    And stdout contains "form[theme]=dark"
    And stdout contains "form[show_sustainer?]=true"
    And stdout contains "web push is not built"
    And the site received 0 account writes

  Scenario: A confirmed settings save is not sent
    When I run "--save-settings --site-theme dark --yes"
    Then it fails
    And stderr contains "are not sent"
    And the site received 0 account writes

  Scenario: A confirmed post report is not sent
    When I run "--report-post POST --reason spam --dry-run"
    Then it succeeds
    And stdout contains "pushEventTo=#js-reportPostActions"
    And stdout contains "form[reason]=spam"
    And the site received 0 account writes
    When I run "--report-post POST --reason spam --yes"
    Then it fails
    And stderr contains "are not sent"
    And the site received 0 account writes

  Scenario: A confirmed user report is not sent
    When I run "--report-user ada --reason uncomradely --dry-run"
    Then it succeeds
    And stdout contains "form=js-reportUserActions-form"
    And stdout contains "cid=3"
    And the site received 0 account writes
    When I run "--report-user ada --reason uncomradely --yes"
    Then it fails
    And stderr contains "are not sent"
    And the site received 0 account writes
