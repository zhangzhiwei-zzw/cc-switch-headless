import { z } from "zod";

// Validate the fields consumers rely on while allowing additive response fields.
const noArgs = z.object({}).strict();
const appArgs = z.object({ app: z.string() }).strict();
const modeArgs = z.object({ appType: z.string() }).strict();
const routeResult = z
  .object({
    kind: z.enum(["remembered", "route", "stack"]),
    clientConfigChanged: z.boolean(),
  })
  .passthrough();

export const rpcContracts = {
  get_current_provider: {
    args: appArgs,
    result: z.string(),
  },
  switch_provider: {
    args: appArgs.extend({ id: z.string() }),
    result: z
      .object({ warnings: z.array(z.string()), route: routeResult.optional() })
      .passthrough(),
  },
  get_pi_current_state: {
    args: noArgs,
    result: z
      .object({
        enabledProviderIds: z.array(z.string()),
        defaultProviderId: z.string().nullable(),
      })
      .passthrough(),
  },
  get_omp_current_state: {
    args: noArgs,
    result: z
      .object({
        enabledProviderIds: z.array(z.string()),
        defaultProviderId: z.string().nullable(),
      })
      .passthrough(),
  },
  get_pi_session_discovery: {
    args: noArgs,
    result: z.discriminatedUnion("status", [
      z.object({ status: z.literal("available") }).passthrough(),
      z
        .object({
          status: z.literal("requires_project_context"),
          configuredPath: z.string(),
        })
        .passthrough(),
      z
        .object({ status: z.literal("unavailable"), reason: z.string() })
        .passthrough(),
    ]),
  },
  get_app_mode: {
    args: modeArgs,
    result: z
      .object({
        mode: z.enum(["direct", "route", "stack"]),
        attached: z.boolean(),
        routeProviderId: z.string().nullable(),
        directProviderId: z.string().nullable(),
      })
      .passthrough(),
  },
  set_proxy_route: {
    args: modeArgs.extend({ providerId: z.string() }),
    result: routeResult,
  },
  get_direct_provider: {
    args: modeArgs,
    result: z.string().nullable(),
  },
  get_opencode_live_provider_ids: {
    args: noArgs,
    result: z.array(z.string()),
  },
  get_opencode_default_model: {
    args: noArgs,
    result: z.string().nullable(),
  },
  set_opencode_default_model: {
    args: z.object({ model: z.string() }).strict(),
    result: z.null(),
  },
  get_openclaw_live_provider_ids: {
    args: noArgs,
    result: z.array(z.string()),
  },
  get_hermes_live_provider_ids: {
    args: noArgs,
    result: z.array(z.string()),
  },
};

export type ContractCommand = keyof typeof rpcContracts;
export type CommandArgs<C extends ContractCommand> = z.input<
  (typeof rpcContracts)[C]["args"]
>;
export type CommandResult<C extends ContractCommand> = z.output<
  (typeof rpcContracts)[C]["result"]
>;

const envelope = z
  .discriminatedUnion("ok", [
    z.object({ ok: z.literal(true), data: z.unknown() }).passthrough(),
    z
      .object({
        ok: z.literal(false),
        error: z.string(),
        code: z.string().optional(),
        requestId: z.string().optional(),
      })
      .passthrough(),
  ])
  .refine(
    (payload) =>
      !payload.ok || Object.prototype.hasOwnProperty.call(payload, "data"),
    { path: ["data"], message: "Successful responses must contain data" },
  );

function validate(schema: z.ZodTypeAny, value: unknown, label: string): void {
  const result = schema.safeParse(value);
  if (!result.success) {
    // Report paths and issue codes, never arguments, credentials or response bodies.
    const details = result.error.issues
      .slice(0, 5)
      .map((issue) => `${issue.path.join(".") || "$"}:${issue.code}`)
      .join(", ");
    throw `E_CONTRACT: ${label} (${details})`;
  }
}

function contractFor(cmd: string) {
  return Object.prototype.hasOwnProperty.call(rpcContracts, cmd)
    ? rpcContracts[cmd as ContractCommand]
    : undefined;
}

export function validateCommandArgs(cmd: string, args: unknown): void {
  const contract = contractFor(cmd);
  if (contract) validate(contract.args, args, `${cmd} request`);
}

export function validateCommandResult(cmd: string, data: unknown): void {
  const contract = contractFor(cmd);
  if (contract) validate(contract.result, data, `${cmd} response`);
}

export function parseInvokeResponse(cmd: string, value: unknown) {
  validate(envelope, value, `${cmd} envelope`);
  return value as z.infer<typeof envelope>;
}
