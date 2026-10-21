Feature: Reply edit
  A reply edit sends validate, then save, once, against the mock.
  It never contacts the live site. A top-level post edit sends nothing.

  Background:
    Given a mock Red Star site

  Scenario: Dry-run prints validate then save and does not connect
    When I run "--edit R1 --body updated --dry-run"
    Then it succeeds
    And stdout contains "dry-run: no connection opened"
    And stdout contains "form[body_input]"
    And stdout does not contain "form[thread_id]"
    And stdout does not contain "form[title]"
    And stdout shows one validate frame and one save frame
    And the site received 0 reply-edit validates and 0 reply-edit saves

  Scenario: A confirmed reply edit sends one validate and one save
    When I run "--edit R1 --body updated --yes"
    Then it succeeds
    And stdout contains "published R1"
    And stdout contains "/rs/post/A1/R1"
    And the site received 1 reply-edit validates and 1 reply-edit saves
    And the reply-edit save value contains "updated"

  Scenario: A top-level post edit does not send
    When I run "--edit A1 --body updated --yes"
    Then it fails
    And stderr contains "a top-level post edit is not sent"
    And the site received 0 reply-edit validates and 0 reply-edit saves

  Scenario: A post older than one hour is refused before a connection
    When I run "--edit R1 --body updated --posted-at 1 --yes"
    Then it fails
    And stderr contains "one hour"
    And the site received 0 reply-edit validates and 0 reply-edit saves
