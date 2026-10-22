import { invoke } from "@tauri-apps/api/core";
import type {
  CommandArgs,
  CommandResult,
  ContractCommand,
} from "./commandSchemas";

/** Typed arguments/results in both builds; the Web transport also validates at runtime. */
export function invokeChecked<C extends ContractCommand>(
  cmd: C,
  ...params: CommandArgs<C> extends Record<string, never>
    ? [args?: CommandArgs<C>]
    : [args: CommandArgs<C>]
): Promise<CommandResult<C>> {
  // This is the single reviewed dynamic invoke used by contracted API wrappers.
  return invoke<CommandResult<C>>(cmd, params[0]);
}
