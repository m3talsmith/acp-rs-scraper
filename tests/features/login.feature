Feature: Logging in with an emailed one-time code
  The scraper submits the saved email over the LiveView, asks for the code,
  and finishes with the sign-in POST, like Firefox would.

  Background:
    Given a mock Red Star site

  Scenario: The right code logs in
    When I run "--nick alice --no-mentions --no-attachments"
    Then it succeeds
    And the site sent a login code to "alice@example.test"
    And stderr contains "Logged in."

  Scenario: A wrong code is rejected
    Given I will enter the code "WRONG1"
    When I run "--nick alice --no-mentions"
    Then it fails
    And stderr contains "sign-in POST failed"
