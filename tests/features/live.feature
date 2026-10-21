Feature: Live updates and site notifications
  Live replies arrive on the LiveView socket. The site Notifications pane is read-only.
  Nothing here contacts the live site.

  Background:
    Given a mock Red Star site

  Scenario: A live reply is stored and a notice is raised
    When I run "--live A1"
    Then it succeeds
    And stdout contains "live	insert	LIVE1	A1	bob	a reply arrived live"
    And stdout contains "live	update	A1	score 9"
    And post "LIVE1" is stored once
    And post "LIVE1" has parent "A1"
    And post "A1" has score 9
    And the database has a notice titled "Live"
    And stdout does not contain "csrf"
    And stdout does not contain "mock-session"

  Scenario: The home feed shows a new-posts chip
    When I run "--live-feed"
    Then it succeeds
    And stdout contains "live	refresh	new posts"
    And stdout contains "live	delete	GONE1"
    And the database has a notice titled "Live"

  Scenario: Site notifications are stored and marked read
    When I run "--site-notifications"
    Then it succeeds
    And stdout contains "site-notification	marked read	clear-app-badge"
    And stdout contains "site-notification	read	ada	N1	A1	ada replied on the strike thread"
    And the database has a notice titled "Site"
    And stdout does not contain "csrf"
    And stdout does not contain "mock-session"
