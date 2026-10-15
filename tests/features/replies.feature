Feature: "Posts and replies" profile filter
  The profile's Filter dialog (form "configure-feed", radio replies=1) shows the
  nick's replies in other people's threads. The scraper loads
  /rs/@nick?replies=1, pages it, and merges it with the Posts feed by post_id.

  Background:
    Given a mock Red Star site

  Scenario: Replies in other threads are stored with their thread as parent
    When I run "--nick alice --no-mentions --no-attachments"
    Then it succeeds
    And the "Posts and replies" URL was requested
    And post "AR1" has parent "B1"
    And post "A5" is stored once
    And stderr contains "posts and replies added 1 items"

  Scenario: Falls back to the Filter form when the URL has no feed
    Given the site's "Posts and replies" URL is broken
    When I run "--nick alice --no-mentions --no-attachments"
    Then it succeeds
    And the profile filter form was used
    And post "AR1" has parent "B1"
    And the database has 7 posts
