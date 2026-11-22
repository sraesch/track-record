import type { UserEvent } from '@testing-library/user-event'

/**
 * Puts text at the end of a field in one input event, as pasting does.
 *
 * `user.type` sends every character as its own key presses, and each one
 * renders the page again (a search field even writes the URL). With React's
 * development build in jsdom that costs about 50 ms per character, several
 * times that on a loaded machine, so long texts ran into the 5 s test
 * timeout. Use this where the text matters, not the single keys; keep
 * `user.type` for keys that do something (`{Enter}`, the character that
 * completes a value).
 */
export async function enterText(
  user: UserEvent,
  field: Element,
  text: string,
) {
  // A click puts the caret at the end of the value, as `user.type` does.
  await user.click(field)
  await user.paste(text)
}
