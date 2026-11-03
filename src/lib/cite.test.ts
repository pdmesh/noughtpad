import { describe, expect, it } from "vitest";
import { answerHtml } from "./cite";

describe("answerHtml", () => {
  it("turns citations into buttons", () => {
    const html = answerHtml("You wrote this [1][2] and that [1, 3].", 3);
    expect(html.match(/data-cite="/g)).toHaveLength(4);
    expect(html).toContain('data-cite="3"');
  });

  it("leaves out-of-range numbers and non-citations alone", () => {
    const html = answerHtml("See [4] and [x] and [0].", 3);
    expect(html).not.toContain("data-cite");
    expect(html).toContain("[4]");
  });

  it("keeps dollar amounts as text, not maths", () => {
    const html = answerHtml("2% of a $10B outcome is $200M [1].", 1);
    expect(html).toContain("$10B outcome is $200M");
    expect(html).not.toContain("katex");
  });

  it("drops raw HTML from the model", () => {
    const html = answerHtml('Hi <img src=x onerror="alert(1)"> [1]', 1);
    expect(html).not.toContain("<img");
    expect(html).not.toContain("onerror");
  });
});
