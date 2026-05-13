// Test: Wiki link parser
import { describe, it, expect } from "vitest";
import { parseWikiLinks } from "../wikiLinks";

describe("parseWikiLinks", () => {
  it("parses basic link [[note]]", () => {
    const result = parseWikiLinks("Some text [[note]] more text");
    expect(result).toHaveLength(1);
    expect(result[0].target).toBe("note");
    expect(result[0].display).toBe("note");
    expect(result[0].heading).toBeNull();
    expect(result[0].position).toEqual({ start: 10, end: 18 });
  });

  it("parses link with display [[note|alias]]", () => {
    const result = parseWikiLinks("Link: [[note|My Note]]");
    expect(result).toHaveLength(1);
    expect(result[0].target).toBe("note");
    expect(result[0].display).toBe("My Note");
    expect(result[0].heading).toBeNull();
  });

  it("parses link with heading [[note#heading]]", () => {
    const result = parseWikiLinks("See [[note#section]]");
    expect(result).toHaveLength(1);
    expect(result[0].target).toBe("note");
    expect(result[0].display).toBe("note");
    expect(result[0].heading).toBe("section");
  });

  it("parses link with heading and display [[note#heading|alias]]", () => {
    const result = parseWikiLinks("[[note#section|My Section]]");
    expect(result).toHaveLength(1);
    expect(result[0].target).toBe("note");
    expect(result[0].display).toBe("My Section");
    expect(result[0].heading).toBe("section");
  });

  it("parses multiple links", () => {
    const result = parseWikiLinks("[[note1]] and [[note2|alias]]");
    expect(result).toHaveLength(2);
    expect(result[0].target).toBe("note1");
    expect(result[1].target).toBe("note2");
    expect(result[1].display).toBe("alias");
  });

  it("returns empty array for no links", () => {
    const result = parseWikiLinks("No links here");
    expect(result).toHaveLength(0);
  });

  it("returns empty array for empty string", () => {
    const result = parseWikiLinks("");
    expect(result).toHaveLength(0);
  });

  it("handles links with spaces", () => {
    const result = parseWikiLinks("[[my note]]");
    expect(result).toHaveLength(1);
    expect(result[0].target).toBe("my note");
  });

  it("skips empty targets", () => {
    const result = parseWikiLinks("[[|display]]");
    expect(result).toHaveLength(0);
  });

  it("skips whitespace-only targets", () => {
    const result = parseWikiLinks("[[  ]]");
    expect(result).toHaveLength(0);
  });

  it("handles complex case [[target#heading|display]]", () => {
    const result = parseWikiLinks("[[my-note#section|My Section]]");
    expect(result).toHaveLength(1);
    expect(result[0].target).toBe("my-note");
    expect(result[0].display).toBe("My Section");
    expect(result[0].heading).toBe("section");
  });

  it("handles heading in display side (ignored)", () => {
    const result = parseWikiLinks("[[note|display#heading]]");
    expect(result).toHaveLength(1);
    expect(result[0].target).toBe("note");
    expect(result[0].display).toBe("display#heading");
    expect(result[0].heading).toBeNull();
  });

  it("handles multiple links with mixed syntax", () => {
    const result = parseWikiLinks("[[note1]] [[note2|alias]] [[note3#heading]]");
    expect(result).toHaveLength(3);
    expect(result[0]).toEqual(expect.objectContaining({ target: "note1", display: "note1" }));
    expect(result[1]).toEqual(expect.objectContaining({ target: "note2", display: "alias" }));
    expect(result[2]).toEqual(expect.objectContaining({ target: "note3", heading: "heading" }));
  });
});
