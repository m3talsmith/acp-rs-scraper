Feature: Truncated feed bodies are completed from the thread page
  Feeds send long posts and comments shortened ("show more" on the site):
  the item has body_truncated true, only its first paragraph and a
  body_text_summary. The full text is on the thread page. Stored posts and
  replies must have the whole body with paragraph breaks kept.

  Background:
    Given a mock Red Star site

  Scenario: A truncated post and reply are stored in full
    When I run "--nick erin --said-by --no-mentions --no-attachments --format html --output $HOME/erin.html"
    Then it succeeds
    And post "E1" has 3 paragraphs ending "Erin para three ends here."
    And post "ER1" has 2 paragraphs ending "Erin reply two ends here."
    And post "E1" has body state "full"
    And stderr contains "2 truncated feed bodies, 2 completed"
    And the file "$HOME/erin.html" contains "<p>Erin para two.</p><p>Erin para three ends here.</p>"

  Scenario: --refetch-truncated backfills rows stored shortened or before 1.26.0
    Given I have run "--nick erin --no-mentions --no-attachments"
    And post "E1" was stored truncated as "truncated"
    And post "ER1" was stored truncated as "unknown"
    When I run "--refetch-truncated --no-attachments"
    Then it succeeds
    And post "E1" has 3 paragraphs ending "Erin para three ends here."
    And post "ER1" has 2 paragraphs ending "Erin reply two ends here."
    And post "ER1" has body state "full"
    And stderr contains "2 completed"
    And the site served 1 thread pages

  Scenario: A row stored truncated is re-read on the next normal scrape
    Given I have run "--nick erin --no-mentions --no-attachments"
    And post "E1" was stored truncated as "truncated"
    When I run "--nick erin --no-mentions --no-attachments"
    Then it succeeds
    And post "E1" has 3 paragraphs ending "Erin para three ends here."
