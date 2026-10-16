Feature: Scraping a nick
  A nick's profile feed is paged to the end, the threads they started are
  fetched for replies, attachments are downloaded, and re-runs are incremental.

  Background:
    Given a mock Red Star site

  Scenario: Profile scrape with paging, threads and attachments
    When I run "--nick @alice --no-mentions"
    Then it succeeds
    And the database has 6 posts by "alice"
    And post "R1" has parent "A1"
    And post "A1" has parent ""
    And the attachments table has 1 rows
    And stdout contains "first post from alice"

  Scenario: Re-signed media links are not downloaded or exported twice
    Given I have run "--nick alice --no-mentions --full"
    When I run "--nick alice --no-mentions --full"
    Then it succeeds
    And the attachments table has 1 rows
    And 1 media files are on disk

  Scenario: Incremental re-scrape fetches nothing new
    Given I have run "--nick alice --no-mentions --no-attachments"
    When I run "--nick alice --no-mentions --no-attachments"
    Then it succeeds
    # Only the profile-details read (first 10 items) pages; the Posts and
    # "Posts and replies" feeds stop at their first, all-known page.
    And the site served at most 2 feed pages via page-next
    And the site served 0 thread pages
    And stderr contains "0 new DB rows"

  Scenario: --full re-crawls everything
    Given I have run "--nick alice --no-mentions --no-attachments"
    When I run "--nick alice --no-mentions --no-attachments --full"
    Then it succeeds
    And the site served 5 thread pages
    And the site served at least 4 feed pages via page-next
    And the database has 7 posts

  Scenario: Mentions are found on the site feed and verified in the thread
    When I run "--nick alice --no-attachments"
    Then it succeeds
    And "B1" is recorded as mentioning "alice"
    And the database has 8 posts

  Scenario: --said-by keeps only the nick's own posts and replies
    When I run "--nick alice --said-by --no-attachments"
    Then it succeeds
    And the site served 0 thread pages
    And post "AR1" has parent "B1"
    And stderr contains "--said-by"

  Scenario: --said-by exports show the nick's own posts and their replies
    When I run "--nick alice --said-by --no-attachments --format json,html --output $HOME/alice.json"
    Then it succeeds
    And the file "$HOME/alice.json" contains '"posts": 5,'
    And the file "$HOME/alice.json" contains '"replies": 1'
    And the file "$HOME/alice.json" contains '"kind": "post"'
    And the file "$HOME/alice.json" contains '"kind": "reply"'
    And the file "$HOME/alice.html" contains '<h2 id="posts">Posts (5)</h2>'
    And the file "$HOME/alice.html" contains '<h2 id="replies">Replies in other threads (1)</h2>'
    And the file "$HOME/alice.html" contains 'class="post kind-post d0" id="post-A1"'
    And the file "$HOME/alice.html" contains 'class="post kind-reply d0" id="post-AR1"'
