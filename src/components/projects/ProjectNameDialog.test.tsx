import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ProjectNameDialog from "./ProjectNameDialog";

const defaultProps = {
  open: true,
  title: "Create project",
  label: "Project name",
  submitLabel: "Create",
  cancelLabel: "Cancel",
  requiredMessage: "Enter a project name.",
  error: null,
  loading: false,
  onSubmit: vi.fn(),
  onCancel: vi.fn(),
};

describe("ProjectNameDialog", () => {
  beforeEach(() => {
    defaultProps.onSubmit.mockReset();
    defaultProps.onCancel.mockReset();
  });

  it("is absent when closed", () => {
    render(<ProjectNameDialog {...defaultProps} open={false} />);

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("shows title, labeled input, cancel button, and submit button when open", () => {
    render(<ProjectNameDialog {...defaultProps} />);

    const dialog = screen.getByRole("dialog");
    expect(dialog).toBeInTheDocument();
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(screen.getByText("Create project")).toBeInTheDocument();
    expect(screen.getByLabelText("Project name")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Create" })).toBeInTheDocument();
  });

  it("shows the required message for blank input and does not call onSubmit", () => {
    render(<ProjectNameDialog {...defaultProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Create" }));

    expect(screen.getByText("Enter a project name.")).toBeInTheDocument();
    expect(defaultProps.onSubmit).not.toHaveBeenCalled();
  });

  it("trims the input value before calling onSubmit", () => {
    render(<ProjectNameDialog {...defaultProps} />);

    fireEvent.change(screen.getByLabelText("Project name"), {
      target: { value: "  Launch Visuals  " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create" }));

    expect(defaultProps.onSubmit).toHaveBeenCalledWith("Launch Visuals");
  });

  it("submits the form when Enter is pressed", () => {
    render(<ProjectNameDialog {...defaultProps} />);

    fireEvent.change(screen.getByLabelText("Project name"), {
      target: { value: "Launch Visuals" },
    });
    fireEvent.keyDown(screen.getByLabelText("Project name"), {
      key: "Enter",
      code: "Enter",
    });

    expect(defaultProps.onSubmit).toHaveBeenCalledWith("Launch Visuals");
  });

  it("renders a form and submits through form submission", () => {
    render(<ProjectNameDialog {...defaultProps} />);

    fireEvent.change(screen.getByLabelText("Project name"), {
      target: { value: "Launch Visuals" },
    });
    const form = screen.getByLabelText("Project name").closest("form");

    expect(form).toBeInTheDocument();
    fireEvent.submit(form!);

    expect(defaultProps.onSubmit).toHaveBeenCalledWith("Launch Visuals");
  });

  it("resets local input from initialName when reopened", () => {
    const { rerender } = render(
      <ProjectNameDialog {...defaultProps} initialName="Brand Storyboards" />,
    );

    fireEvent.change(screen.getByLabelText("Project name"), {
      target: { value: "Dirty draft" },
    });
    expect(screen.getByLabelText("Project name")).toHaveValue("Dirty draft");

    rerender(
      <ProjectNameDialog {...defaultProps} open={false} initialName="Brand Storyboards" />,
    );
    rerender(
      <ProjectNameDialog {...defaultProps} initialName="Launch Visuals" />,
    );

    expect(screen.getByLabelText("Project name")).toHaveValue("Launch Visuals");
  });

  it("does not reset in-progress edits when initialName changes while open", () => {
    const { rerender } = render(
      <ProjectNameDialog {...defaultProps} initialName="Brand Storyboards" />,
    );

    fireEvent.change(screen.getByLabelText("Project name"), {
      target: { value: "Dirty draft" },
    });
    rerender(
      <ProjectNameDialog {...defaultProps} initialName="Launch Visuals" />,
    );

    expect(screen.getByLabelText("Project name")).toHaveValue("Dirty draft");
  });

  it("calls onCancel from the cancel button", () => {
    render(<ProjectNameDialog {...defaultProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(defaultProps.onCancel).toHaveBeenCalledTimes(1);
  });

  it("disables input and buttons while loading", () => {
    render(<ProjectNameDialog {...defaultProps} loading />);

    expect(screen.getByLabelText("Project name")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Create" })).toBeDisabled();
  });

  it("renders error as a visible alert", () => {
    render(<ProjectNameDialog {...defaultProps} error="Unable to create project." />);

    expect(screen.getByRole("alert")).toHaveTextContent("Unable to create project.");
  });

});
