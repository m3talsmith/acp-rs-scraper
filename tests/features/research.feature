Feature: Research projects
  Collect posts, replies, search results, local files and web pages into
  named projects with notes, and export them together.

  Background:
    Given a mock Red Star site
    And I have run "--nick alice --no-mentions"

  Scenario: Create and list projects
    When I run "--research-create strike --research-description railroads"
    Then it succeeds
    When I run "--research-create Strike"
    Then it fails
    And stderr contains "already exists"
    When I run "--research-list --format json"
    Then it succeeds
    And stdout contains "railroads"

  Scenario: Add posts, replies and search results
    When I run "--research-add strike --post-ids A5,R1,NOPE1234 --note key"
    Then it succeeds
    And stderr contains "created"
    And stderr contains "2 added, 0 already there"
    And stderr contains "post NOPE1234 is not in the database"
    And research project "strike" has a "post" item "A5"
    And research project "strike" has a "reply" item "R1"
    When I run "--research-add strike --search general --nicks alice"
    Then it succeeds
    And research project "strike" has a "search-result" item "A5"
    And research project "strike" has 3 items
    When I run "--research-add strike --post-ids A5"
    Then it succeeds
    And stderr contains "0 added, 1 already there"

  Scenario: Add a local file and a web page, then export everything
    Given a local file "$HOME/notes.md" containing "# Notes about the strike"
    When I run "--research-add strike --file $HOME/notes.md --note mine"
    Then it succeeds
    And research project "strike" has a "local-file" item "$HOME/notes.md"
    When I run "--research-add strike --url $SITE/web/article"
    Then it succeeds
    And research project "strike" has a "web-url" item "$SITE/web/article"
    And the web page was fetched without the site's cookies
    When I run "--research-add strike --post-ids A1"
    Then it succeeds
    When I run "--research-show strike"
    Then it succeeds
    And stdout contains "The Great Railroad Strike"
    When I run "--research-export strike --export $HOME/strike.json --format json,html"
    Then it succeeds
    And the file "$HOME/strike.json" contains "Workers stopped the trains in Martinsburg."
    And the file "$HOME/strike.json" does not contain "tracking"
    And the file "$HOME/strike.json" contains "first post from alice"
    And the file "$HOME/strike.html" contains "Research: strike"
    And the file "$HOME/strike.html" contains "Notes about the strike"
    When I run "--research-export strike --export $HOME/strike.tar.gz --compress --tar-gz"
    Then it succeeds
    And the file "$HOME/strike.tar.gz" exists

  Scenario: Research flags check their inputs
    When I run "--research-add strike"
    Then it fails
    And stderr contains "needs something to add"
    When I run "--research-export nope --export $HOME/x.json"
    Then it fails
    And stderr contains "no research project"
    When I run "--file x.md"
    Then it fails

  Scenario: The TUI adds a post to a research project with a side job
    When I add "A2" to research project "TUI picks" from the TUI
    Then it succeeds
    And research project "TUI picks" has a "post" item "A2"
