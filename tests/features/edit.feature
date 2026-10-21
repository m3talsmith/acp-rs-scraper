Feature: Reply edit
  A reply edit sends validate, then save, once, against the mock.
  It never contacts the live site. A leading quote is kept. A top-level post edit sends both fields.

  Background:
    Given a mock Red Star site

  Scenario: Dry-run prints validate then save and does not connect
    When I run "--edit R1 --body updated --dry-run"
    Then it succeeds
    And stdout contains "dry-run: no connection opened"
    And stdout contains "form[body_input]"
    And stdout does not contain "form[thread_id]"
    And stdout does not contain "form[title]"
    And stdout contains "existing quote"
    And stdout contains "is not shown"
    And stdout shows one validate frame and one save frame
    And the site received 0 reply-edit validates and 0 reply-edit saves

  Scenario: A confirmed reply edit sends one validate and one save
    When I run "--edit R1 --body updated --yes"
    Then it succeeds
    And stdout contains "published R1"
    And stdout contains "/rs/post/A1/R1"
    And the site received 1 reply-edit validates and 1 reply-edit saves
    And the reply-edit save value contains "updated"

  Scenario: A top-level post edit sends the mounted title and the new body
    When I run "--edit A1 --body updated --yes"
    Then it succeeds
    And stdout contains "published A1"
    And stdout contains "/rs/post/A1"
    And the site received 0 reply-edit validates and 0 reply-edit saves
    And the site received 1 post-edit validates and 1 post-edit saves
    And the post-edit save value contains "Kept"
    And the post-edit save value contains "updated"

  Scenario: A reply edit keeps a leading quote
    When I run "--edit QR1 --body updated --yes"
    Then it succeeds
    And stdout contains "published QR1"
    And stdout contains "/rs/post/QTHREAD/QR1"
    And the reply-edit save value contains "postRef"
    And the reply-edit save value contains "QPOST"
    And the reply-edit save value contains "updated"

  Scenario: A post older than one hour is refused before a connection
    When I run "--edit R1 --body updated --posted-at 1 --yes"
    Then it fails
    And stderr contains "one hour"
    And the site received 0 reply-edit validates and 0 reply-edit saves
