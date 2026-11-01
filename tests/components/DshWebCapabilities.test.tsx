import { fireEvent, render, screen } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AddProviderDialog } from "@/components/providers/AddProviderDialog";
import { ProviderList } from "@/components/providers/ProviderList";
import { dshApi } from "@/lib/api/dsh";
import { createTestQueryClient } from "../utils/testQueryClient";

vi.mock("@/lib/capabilities", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/capabilities")>();
  return {
    ...actual,
    supports: (feature: Parameters<typeof actual.supports>[0]) =>
      feature !== "pageAuth" && actual.supports(feature),
  };
});
vi.mock("@/lib/api/dsh", () => ({
  dshApi: { accountStatus: vi.fn() },
}));
vi.mock("@/components/JsonEditor", () => ({ default: () => null }));

describe("DeepSeek Harness Web account capability", () => {
  beforeEach(() => vi.clearAllMocks());

  it("keeps provider creation available without advertising unsupported account login", async () => {
    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <AddProviderDialog
          open
          appId="dsh"
          onOpenChange={vi.fn()}
          onSubmit={vi.fn()}
        />
      </QueryClientProvider>,
    );

    expect(
      screen.queryByRole("button", { name: "dsh.account.presetName" }),
    ).not.toBeInTheDocument();
    fireEvent.click(await screen.findByText("providerPreset.custom"));
    expect(
      await screen.findByRole("button", { name: "common.add" }),
    ).toBeVisible();
    expect(dshApi.accountStatus).not.toHaveBeenCalled();
  });

  it("does not query an unsupported account endpoint when listing providers", () => {
    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <ProviderList
          providers={{}}
          currentProviderId=""
          appId="dsh"
          onSwitch={vi.fn()}
          onEdit={vi.fn()}
          onDelete={vi.fn()}
          onDuplicate={vi.fn()}
          onOpenWebsite={vi.fn()}
          onCreate={vi.fn()}
        />
      </QueryClientProvider>,
    );

    expect(dshApi.accountStatus).not.toHaveBeenCalled();
  });
});
