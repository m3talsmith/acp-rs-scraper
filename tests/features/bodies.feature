Feature: Post bodies, profile images and compressed exports
  Feed items don't always carry a ProseMirror body: some send HTML or plain
  strings, and some reply items arrive with no body at all. Every stored post
  must still have its text, the profile photo is saved with the profile, and a
  compressed export leaves only the archive behind.

  Background:
    Given a mock Red Star site

  Scenario: Replies with HTML or missing feed bodies are stored with their text
    When I run "--nick dave --no-mentions --no-attachments"
    Then it succeeds
    And post "DR1" has content "dave & the html body"
    And post "DR2" has content "dave full reply from the thread"

  Scenario: Blank rows from an older scrape are repaired on the next run
    Given I have run "--nick dave --no-mentions --no-attachments"
    And post "DR2" was stored blank
    When I run "--nick dave --no-mentions --no-attachments"
    Then it succeeds
    And post "DR2" has content "dave full reply from the thread"
    And post "DR2" is stored once

  Scenario: The profile image is downloaded and shown in the HTML export
    When I run "--nick dave --no-mentions --format html --output $HOME/dave.html"
    Then it succeeds
    And stderr contains "profile image"
    And the file ".acp/ai/data/attachments/profiles/dave.png" exists
    And the file "$HOME/attachments/profiles/dave.png" exists
    And the file "$HOME/dave.html" shows the profile image "attachments/profiles/dave.png"

  Scenario: A compressed export removes its uncompressed folder
    Given I have run "--nick dave --no-mentions --no-attachments"
    When I run "--export $HOME/out/dave.tar.gz --compress --tar-gz"
    Then it succeeds
    And the file "$HOME/out/dave.tar.gz" exists
    And the file "$HOME/out/dave" does not exist
