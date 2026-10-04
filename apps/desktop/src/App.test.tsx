import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { useStageCue } from "./store";

describe("StageCue editor", () => {
  beforeEach(() => useStageCue.setState({ page: "editor", selectedSectionId: undefined, history: [], future: [], routingConfirmed: false }));
  it("renames and resizes a selected section", () => {
    render(<App/>); fireEvent.click(screen.getByRole("button", {name:/Intro/}));
    fireEvent.change(screen.getByLabelText("Name"), {target:{value:"Opening"}});
    fireEvent.change(screen.getByLabelText("Length"), {target:{value:"6"}});
    expect(screen.getByRole("button", {name:/Opening/})).toBeInTheDocument();
    expect(useStageCue.getState().project.sections[0].lengthBars).toBe(6);
  });
  it("requires and records routing confirmation", () => {
    render(<App/>); fireEvent.click(screen.getByRole("button", {name:"Mixer & routing"}));
    expect(screen.getByText(/Confirmation required/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", {name:"Confirm routing"}));
    expect(screen.getByText(/Routing confirmed/)).toBeInTheDocument();
  });
  it("does not auto-start from setlist", () => {
    render(<App/>); fireEvent.click(screen.getByRole("button", {name:"Setlist"}));
    expect(screen.getByText(/Manual advance is on/)).toBeInTheDocument();
    expect(useStageCue.getState().playing).toBe(false);
  });
  it("edits click and voice cue tracks", () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "⌁ Click" }));
    fireEvent.click(screen.getByRole("button", { name: "Disable click" }));
    expect(useStageCue.getState().project.click.enabled).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "◉ Voice cues" }));
    fireEvent.change(screen.getByLabelText("Spoken text"), { target: { value: "Guitar solo" } });
    fireEvent.change(screen.getByLabelText("Bar"), { target: { value: "9" } });
    fireEvent.click(screen.getByRole("button", { name: "Add generated cue" }));
    expect(useStageCue.getState().project.events.at(-1)).toMatchObject({ type: "voice-cue", text: "Guitar solo" });
  });
});
