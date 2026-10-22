Feature: Captured interactions
  Vote, subscribe, site bookmark, delete, and a quote repost send the captured frames.
  Unsubscribe and clearing a bookmark do not. The site does not allow a plain repost. Share is not a write.
  The mock never contacts the live site.

  Background:
    Given a mock Red Star site

  Scenario: An upvote sends state 1
    When I run "--vote A1 --yes"
    Then it succeeds
    And stdout contains "voted A1 state 1"
    And stdout contains "vote upvote"
    And the site received 1 vote hooks

  Scenario: Clearing a vote sends state 0
    When I run "--vote A1 --vote-state 0 --yes"
    Then it succeeds
    And stdout contains "vote none"
    And the site received 1 vote hooks

  Scenario: A subscribe sends state 1 and unsubscribe is refused
    When I run "--subscribe A1 --yes"
    Then it succeeds
    And stdout contains "subscribed A1"
    And stdout contains "subscribed true"
    And the site received 1 subscribe hooks

  Scenario: Unsubscribe names the missing frame
    When I run "--subscribe A1 --subscribe-state 0 --dry-run"
    Then it fails
    And stderr contains "subscribe hook with state"
    And stderr contains "Nothing was sent"
    And the site received 0 subscribe hooks

  Scenario: A site bookmark sends state 1
    When I run "--site-bookmark A1 --yes"
    Then it succeeds
    And stdout contains "bookmarked A1"
    And stdout contains "bookmarked true"

  Scenario: A delete sends delete-post and names the tombstone
    When I run "--delete A1 --yes"
    Then it succeeds
    And stdout contains "deleted A1"
    And stdout contains "tombstone"
    And the site received 1 delete hooks

  Scenario: A quote repost sends the captured fields
    When I run "--repost A1 --repost-title Noted --body updated --yes"
    Then it succeeds
    And stdout contains "published RP1"
    And stdout contains "/rs/post/RP1"
    And the site received 1 repost validates and 1 repost saves

  Scenario: A plain repost is refused before a connection
    When I run "--repost A1 --repost-title Noted --yes"
    Then it fails
    And stderr contains "does not allow a plain repost"
    And stderr contains "Nothing was sent"
    And the site received 0 repost validates and 0 repost saves
    When I run "--repost A1 --repost-title Noted --dry-run"
    Then it fails
    And stderr contains "does not allow a plain repost"
    And stderr contains "--body"
    And the site received 0 repost validates and 0 repost saves
