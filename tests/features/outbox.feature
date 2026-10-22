Feature: Outbox
  Drafts and the send queue stay on this machine until a confirmed send.
  A dry-run prints the publish frames and does not connect.
  The mock stands in for the site. These scenarios never contact acp.us.

  Background:
    Given a mock Red Star site

  Scenario: A draft is versioned, queued, and not sent without a confirm
    When I run "--draft-save --draft-id D1 --draft-title Hello --body notes"
    Then it succeeds
    And stdout contains "draft D1 state=draft versions=1"
    When I run "--draft-save --draft-id D1 --draft-title Hello --body revised"
    Then it succeeds
    And stdout contains "draft D1 state=draft versions=2"
    When I run "--draft-list"
    Then it succeeds
    And stdout contains "draft D1 state=draft versions=2"
    When I run "--outbox-queue D1"
    Then it succeeds
    And stdout contains "outbox Q-D1 status=queued action=new_post"
    When I run "--outbox-list"
    Then it succeeds
    And stdout contains "outbox Q-D1 status=queued action=new_post"
    When I run "--outbox-send Q-D1 --dry-run"
    Then it succeeds
    And stdout contains "dry-run: no connection opened"
    And stdout contains "validate"
    And stdout contains "save"
    And stdout does not contain "https://acp.us"
    When I run "--outbox-send Q-D1"
    Then it succeeds
    And stdout contains "nothing sent"
    When I run "--outbox-cancel Q-D1"
    Then it succeeds
    And stdout contains "outbox Q-D1 status=cancelled"

  Scenario: Keep local stays off the queue
    When I run "--draft-save --keep-local --draft-id L1 --draft-title Kept --body stays"
    Then it succeeds
    And stdout contains "draft L1 state=local versions=1"
    When I run "--draft-list"
    Then it succeeds
    And stdout contains "state=local"
