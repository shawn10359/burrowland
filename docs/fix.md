# `fix` Branch Commit Summary

Based on the commit order in `main..HEAD`, this document organizes each commit on this branch by its **changes** and **intent**.

> Base branch: `main`
> 
> Commit range: `fc83fb4` → `d9737d4`

---

## 1. `fc83fb4` — Validate Ref V1 aggregated swap actions at parse time.

This commit aims to fix the root cause of this incident, the flawed slippage protection in margin trading swap verification logic.  

### Changes
- Added `validate_actions()` to `RefV1TokenReceiverMessage` in `contracts/contract/src/margin_trading.rs`.
- The validation now runs immediately after parsing `SwapIndication`, instead of relying on later execution stages to assume the structure is valid.
- The new validation rules mainly include:
  - the action list must not be empty;
  - the first hop must explicitly provide `amount_in`;
  - within a single route, each hop must connect strictly to the next one, meaning the next `token_in` must equal the previous `token_out`;
  - mid-route hops must not explicitly provide another `amount_in`;
  - once a route reaches the final `token_out`, any following action must start a new route from the original `token_in`.
- Added unit tests covering:
  - valid aggregation for a single route;
  - valid aggregation for multiple safe routes;
  - invalid routes that loop or revisit the final output token;
  - invalid routes that specify `amount_in` again in the middle of a route.

### Intent
- Move Ref V1 aggregated swap path validation forward to the parsing stage, so structurally invalid swap paths are rejected as early as possible.
- Prevent malformed route structures from being used to create loops or repeatedly revisit the final output token, which could incorrectly inflate aggregated `min_amount_out`.
- Make the input constraints of Ref V1 aggregated swaps explicit and reduce reliance on implicit assumptions in later execution logic.

---

## 2. `60f79c6` — Add owner-controlled margin action switches.

This commit aims to turn off margin trading in an elegant way, by switching off open-margin action while leaving existing margin position to be closed by their owner or liquidated eventually.  

### Changes
- In `contracts/contract/src/margin_actions.rs`:
  - added `action_type()` to `MarginAction`, mapping concrete actions to `MarginActionType`;
  - loaded `margin_config` in `internal_margin_execute()` and checked whether each action is enabled before execution.
- Introduced an `enabled_actions` configuration field in `contracts/contract/src/margin_config.rs` and related initialization/upgrade paths.
- Added upgrade compatibility structures such as `MarginConfigV2` and `ContractV0150` in `contracts/contract/src/legacy.rs`, so older state can migrate to the new structure with `enabled_actions`.
- Initialized `enabled_actions: 0` in `contracts/contract/src/lib.rs`.
- In `contracts/contract/tests/workspace_env/setup.rs`, explicitly called `batch_enable_margin_actions` after test deployment to enable all margin actions and keep existing tests working.
- Updated the upgrade test baseline from `0.15.9 -> 0.16.0`, and pinned the previous wasm to `releases/burrowland_0.15.9.wasm`.
- Added a saved copy of `releases/burrowland_0.15.9.wasm` as upgrade test input.

### Intent
- Add an owner-controlled global/per-action switch for margin operations, so the protocol can quickly disable high-risk margin behaviors during risky periods.
- Provide finer-grained controls for maintenance, incident response, and staged re-enablement of features.
- Ensure state migration is complete so the new configuration field does not break upgrades from older contract versions.

---

## 3. `4265755` — Add owner contract token withdrawal.

This commit aims to provide a dedicated withdrawal interface to process specific hacker refund tokens.   

### Changes
- Added an owner-only method in `contracts/contract/src/config.rs`:
  - `withdraw_contract_token_to_owner(token_id, ft_amount, memo)`
- This method:
  - requires `assert_one_yocto()`;
  - requires the caller to be the owner;
  - uses the FT standard interface `ft_transfer` to move a specified amount of an FT from the contract account to the owner.

### Intent
- Give the owner a direct administrative method to withdraw FT balances held by the contract account.
- This is useful for residual balances, mistaken transfers, or operational fund collection, without requiring a heavier governance or upgrade path.

---

## 4. `d243bcb` — Accrue margin liquidation holding fees to protocol fees.

This commit aims to fix an uncollected holding position fee issue.    

### Changes
- In `contracts/contract/src/margin_position.rs`, during margin liquidation:
  - `hp_fee` was already included in the amount to be repaid/deducted;
  - now `hp_fee` is also added to the corresponding asset’s `prot_fee`.
- Strengthened tests in `contracts/contract/tests/margin_trading.rs` by:
  - setting a high `holding_position_fee_rate`;
  - using `fast_forward` to ensure HP fees actually accrue over time;
  - verifying across multiple liquidation paths that `prot_fee` increases after liquidation.

### Intent
- Make the ownership of holding position fees explicit in liquidation flows: they should accrue as protocol revenue rather than only being involved in debt settlement.
- Avoid a situation where liquidation mathematically accounts for the fee but does not record it as protocol income, leading to understated fee accounting.

---

## 5. `9b970ce` — Add maintenance mode access controls.

This commit aims to add a brand new maintenance mode for secure contract state recovery. In this mode:  
- public users interactions are blocked;
- public liquidation and force-close are blocked;
- only whitelisted near accounts can access to verify the contract functionality before reopening;
- only whitelisted liquidator can initiate liquidation and force-close, to solve the hacker’s debt under team’s control;
- Support DAO management actions, to adjust token config before reopening;
- Support special state reset action to adjust supply/borrow/margin_debt share prices.


### Changes
- In `contracts/contract/src/config.rs`, added:
  - the `ContractMode` enum: `Running` / `Maintenance`;
  - the `Config.contract_mode` field;
  - the owner-only method `update_contract_mode()`;
  - two guard functions:
    - `assert_operation_allowed_in_current_mode()`
    - `assert_liquidation_allowed_in_current_mode()`
- In `contracts/contract/src/maintenance_mode.rs`, added maintenance-mode whitelist logic:
  - append/remove/get whitelist;
  - support for suffix wildcards in the form `*.suffix`.
- In `contracts/contract/src/fungible_token.rs`, added maintenance-mode checks to nearly all user entry operations, including:
  - normal `Execute`;
  - `ExecuteWithPyth`;
  - `DepositToReserve`;
  - `DepositToMargin`;
  - `MarginExecute`;
  - `MarginExecuteWithPyth`;
  - `OnlyIncreaseCollateral`;
  - `OnlyRepay`;
  - `ClientEchoDeposit`.
- In `contracts/contract/src/margin_actions.rs`:
  - added maintenance-mode checks to `margin_execute()`;
  - required callers to be in the reliable liquidator whitelist for liquidation during maintenance mode.
- In `contracts/contract/src/legacy.rs`, added `ConfigV4` and the migration path from older config versions; older versions default into `Maintenance` after upgrade.
- Added the `MAINTENANCE_MODE_WHITELIST` storage key.
- Explicitly set newly deployed test instances to `Running` in test setup.
- Added tests in `contracts/contract/tests/liquidation.rs` verifying that:
  - maintenance mode blocks normal user deposits;
  - whitelisted accounts can continue operating;
  - only reliable liquidators can liquidate during maintenance mode.

### Intent
- Provide a dedicated maintenance operating mode for the protocol, allowing user-facing entry points to be restricted during incidents, migrations, or manual repair work.
- Allow only whitelisted accounts and reliable liquidators to retain necessary capabilities, limiting risk propagation while preserving minimal operational control.
- Default upgraded old state into `Maintenance` as a safety-first choice: restrict first, then reopen manually once confirmed.

---

## 6. `ee2555a` — Add owner-only asset interest rollback interface.

This commit includes the actual implementation of share price reset interface.    

### Changes
- Added `internal_get_asset_raw()` / `internal_unwrap_asset_raw()` in `contracts/contract/src/asset.rs` to read the underlying asset state directly without going through the cache.
- Added two new structs in `contracts/contract/src/config.rs`:
  - `AssetInterestRollbackExpected`: the expected pre-call state snapshot;
  - `AssetInterestRollbackTarget`: the target state after rollback.
- Added the owner-only method `rollback_asset_interest()`:
  - requires `assert_one_yocto()`;
  - requires owner privileges;
  - requires the contract to be in `Maintenance`;
  - first checks that the current key asset state matches `expected` exactly;
  - if the checks pass, rewrites fields such as balance/reserved/beneficiary_fees/unit_acc_hp_interest/last_update_timestamp using `target`.
- Added unit tests in `contracts/contract/src/lib.rs` verifying:
  - successful rollback in the normal case;
  - rejection outside maintenance mode;
  - rejection when the expected state does not match;
  - rejection of reusing the same `expected` state twice.

### Intent
- Provide an owner-only interface for manually rolling back asset interest state, usable only while the contract is in maintenance mode.
- Use an `expected -> target` compare-and-swap style flow to reduce the risk of accidental repair mistakes: writes are allowed only if on-chain state matches the operator’s expectations exactly.
- This is intended more as an incident-repair/accounting-correction tool than as a normal operational interface.

---

## 7. `0b9731c` — Refactor margin action preflight gating.

This commit aims to improve margin action switch checks.    

### Changes
- Extracted `assert_margin_actions_enabled()` in `contracts/contract/src/margin_actions.rs`.
- Moved the “is this action enabled?” check from inside the execution loop into a unified preflight stage, so the full action set is validated before any action starts executing.

### Intent
- Make margin action switch checks clearer and better aligned with the idea of preflight gating.
- Avoid failing halfway through execution because a later action is disabled, which reduces the complexity of partial execution scenarios.

---

## 8. `54da5ed` — Document SwapReference mode check behavior.

This commit marks interfaces that won't get affected by the Maintenance mode and give an explanation.  

### Changes
- Added a comment in the `TokenReceiverMsg::SwapReference` branch of `contracts/contract/src/fungible_token.rs`.
- The comment explains that `SwapReference` belongs to the async callback leg of a position action that already passed mode checks at initiation time, so mode checks are not repeated here.

### Intent
- Explain why this path does not perform the same maintenance-mode checks as other entry points.
- Prevent future maintainers from treating this as an omission or incorrectly adding repeated checks during the async callback phase, which could break completion of already-started flows.

---

## 9. `674c157` — Add one-shot owner withdrawal path.

This commit aims to secure the dedicated withdrawal interface to one-time-use.   

### Changes
- In `contracts/contract/src/config.rs`:
  - removed/replaced the generic `withdraw_contract_token_to_owner(token_id, ft_amount, memo)` interface;
  - replaced it with a fixed `execute_owner_one_shot_withdrawal()` path.
- Added one-time execution guards:
  - `assert_owner_one_shot_withdraw_unused()`
  - `mark_owner_one_shot_withdraw_used()`
- When this one-shot path executes, it:
  - requires `assert_one_yocto()`;
  - requires owner privileges;
  - requires that it has not been used before;
  - transfers a fixed amount of tokens to a fixed FT contract address;
  - also transfers a fixed amount of NEAR to the owner;
  - writes the “used” marker before performing the transfers.
- Added `OWNER_ONE_SHOT_WITHDRAW_USED` in `contracts/contract/src/storage_keys.rs`.
- Added unit tests verifying:
  - the first execution marks storage as used;
  - the path cannot be executed twice;
  - non-owners cannot execute it.

### Intent
- Narrow the original generic owner-withdrawal capability into a special-purpose path with a fixed target, fixed amount, and one-time-only execution.
- This clearly reduces the scope of administrative power, avoiding a standing generic capability for arbitrary owner withdrawals.
- Writing the “used” marker before the cross-contract transfers also ensures the path cannot be triggered again even if a later transfer step fails.

---

## 10. `16d25d4` — Add RunningWithoutPublicLiquidation contract mode.

This commit aims to keep the contract running for normal operations while disabling public liquidation and force-close access.  

### Changes
- In `contracts/contract/src/config.rs`, expanded `ContractMode` from `Running / Maintenance` to `Running / RunningWithoutPublicLiquidation / Maintenance`.
- Updated `assert_liquidation_allowed_in_current_mode()` so that:
  - `Running` still allows public liquidation;
  - `RunningWithoutPublicLiquidation` requires the caller to be in the reliable liquidator whitelist;
  - `Maintenance` continues to require the reliable liquidator whitelist for liquidation paths.
- Added unit tests in `contracts/contract/src/config.rs` covering:
  - normal operations remain allowed in `RunningWithoutPublicLiquidation`;
  - non-whitelisted liquidators are rejected in that mode;
  - `Maintenance` liquidation effectively requires both the maintenance whitelist and reliable liquidator whitelist.

### Intent
- Introduce an intermediate operating mode between fully open operation and full maintenance lockdown.
- Allow the protocol to continue serving standard user actions while temporarily restricting liquidation and force-close authority to trusted liquidators only.
- Make the dual-whitelist behavior in `Maintenance` explicit and regression-tested.

---

## 11. `afdabcd` — Add pause operator maintenance controls.

This commit aims to delegate emergency maintenance activation to a dedicated operator without granting full owner control.  

### Changes
- In `contracts/contract/src/config.rs`, added:
  - `Config.pause_operator_id: Option<AccountId>`;
  - `assert_owner_or_pause_operator()`;
  - the owner-only config method `update_pause_operator_id()`;
  - the `pause_contract()` admin method, which allows the owner or pause operator to set `contract_mode = Maintenance` directly.
- Kept `update_contract_mode()` owner-only, so the pause operator can enter maintenance but cannot freely switch modes.
- Updated config migration defaults in `contracts/contract/src/legacy.rs` so `ConfigV0..V4 -> Config` always backfill `pause_operator_id: None`.
- Updated current `Config { ... }` initialization sites in:
  - `contracts/contract/src/lib.rs`;
  - `contracts/contract/src/margin_trading.rs`;
  - `contracts/contract/tests/workspace_env/setup.rs`.
- Added unit tests in `contracts/contract/src/config.rs` verifying:
  - owner can set and clear `pause_operator_id`;
  - non-owner cannot update it;
  - owner and pause operator can call `pause_contract()`;
  - unrelated accounts cannot call `pause_contract()`;
  - pause operator still cannot use the owner-only `update_contract_mode()` interface.

### Intent
- Add a narrowly scoped emergency role that can pause the protocol into `Maintenance` without exposing broader owner powers.
- Separate “who can trigger the emergency stop” from “who fully administers contract mode,” which reduces operational coupling and privilege scope.
- Preserve upgrade compatibility by making the new config field optional and defaulting old state to `None` during migration.

---

## 12. `3b7a9bf` — Restrict StopMTPosition in liquidation-gated modes.

This commit aims to make the take-profit / stop-loss entrypoint follow the same contract-mode liquidation restrictions as forced liquidation.  

### Changes
- In `contracts/contract/src/margin_actions.rs`, added `assert_liquidation_allowed_in_current_mode(account_id)` inside the `MarginAction::StopMTPosition` branch.
- As a result, `StopMTPosition` now passes through the same contract-mode gate already used by liquidation / force-close style flows.

### Intent
- Treat `StopMTPosition` as a liquidation-adjacent entrypoint rather than a normal unrestricted user action.
- Ensure that in `RunningWithoutPublicLiquidation` and `Maintenance`, the stop-profit / stop-loss path cannot bypass the same reliable-liquidator restrictions that already apply to liquidation.
- Close the policy gap where public users could still reach a liquidation-like path through `StopMTPosition` even after public liquidation was disabled.

---

## 13. `fb78c35` — Gate margin deposits behind enabled_actions.

This commit aims to make margin deposit entry flows follow the same owner-controlled action switch model as the rest of margin operations.  

### Changes
- In `contracts/contract/src/margin_config.rs`:
  - added `Deposit` to `MarginActionType`;
  - assigned it its own bit in `enabled_actions`;
  - included it in `get_enabled_margin_actions()`;
  - updated margin action config tests to cover the new action.
- In `contracts/contract/src/fungible_token.rs`:
  - added `assert_margin_deposit_enabled()` as a shared helper;
  - required `MarginActionType::Deposit` to be enabled before processing:
    - `TokenReceiverMsg::DepositToMargin`;
    - `TokenReceiverMsg::MarginExecute`;
    - `TokenReceiverMsg::MarginExecuteWithPyth`.
- Added unit tests covering:
  - `DepositToMargin` fails when `Deposit` is disabled;
  - `DepositToMargin` succeeds when `Deposit` is enabled;
  - `MarginExecute` also fails when `Deposit` is disabled.

### Intent
- Extend `enabled_actions` so it controls not only explicit margin actions executed inside `internal_margin_execute()`, but also the FT-based margin deposit entry flows that deposit funds before margin actions begin.
- Give operators a consistent way to disable new margin funding while still keeping finer-grained control over other margin behaviors.
- Reduce the chance that margin deposits remain unintentionally available when the protocol expects all margin entry capability to be switchable.

---

## 14. `15cc0fe` — Bind margin swap callbacks to pending action metadata.

This commit aims to harden the return leg of margin swap callbacks so the protocol only accepts the exact callback it originally initiated.  

### Changes
- Added `contracts/contract/src/margin_pending_action.rs` with a `PendingMarginAction` record stored per pending position action.
- The pending action now records:
  - expected `dex_id`;
  - expected output token;
  - expected `op`;
  - expected `amount_in`;
  - expected `min_amount_out`;
  - expected `liquidator_id` when applicable.
- In `contracts/contract/src/margin_position.rs`:
  - wrote pending action records when opening a margin position;
  - wrote pending action records when decreasing / closing / liquidating / force-closing / stop-closing a margin position;
  - removed pending action records during the revert path of `callback_dex_trade()`.
- In `contracts/contract/src/fungible_token.rs`, strengthened the `TokenReceiverMsg::SwapReference` branch so it now validates:
  - callback DEX identity;
  - callback token identity;
  - `amount_in`;
  - `op`;
  - `liquidator_id`;
  - `action_ts`;
  - final `min_amount_out` sufficiency.
- If any callback mismatch is detected, the contract:
  - logs the incident;
  - moves into maintenance mode;
  - returns the received FT amount back to the DEX.
- Added focused unit tests for:
  - wrong token callbacks;
  - wrong DEX callbacks;
  - mismatched callback payloads;
  - valid callback success.

### Intent
- Bind the incoming swap callback to the exact swap request the protocol originally dispatched.
- Prevent a registered DEX or wrong token contract from completing another position’s pending action with forged callback data.
- Add a protocol-side final settlement check instead of trusting the DEX callback shape by default.
- Fail closed by pausing the contract if the callback stream shows signs of inconsistency.

---

## 15. `06b2c75` — Narrow ft_on_transfer deposit gating for swap callbacks.

This commit aims to reduce the scope of generic FT deposit gating so it applies only to real entry flows, not to already-authorized margin business callbacks.  

### Changes
- In `contracts/contract/src/fungible_token.rs`:
  - removed the unconditional top-level `asset.config.can_deposit` assertion from the beginning of `ft_on_transfer()`;
  - introduced `assert_ft_transfer_deposit_enabled()` as an explicit helper for deposit-style FT entrypoints;
  - applied that helper only to the real FT entry branches, including:
    - empty-message deposit flow;
    - `Execute` / `ExecuteWithPyth`;
    - `DepositToReserve` / `DepositToMargin`;
    - `MarginExecute` / `MarginExecuteWithPyth`;
    - `OnlyIncreaseCollateral` / `OnlyRepay`;
    - `ClientEchoDeposit`.
- Left `TokenReceiverMsg::SwapReference` outside this generic `can_deposit` gate.
- Kept the ordering clear in each gated branch:
  - contract-mode check first;
  - deposit-capability check second.

### Intent
- Treat `SwapReference` as an async business callback for an already-started margin workflow rather than as a fresh FT deposit entrypoint.
- Preserve existing deposit restrictions for normal FT entry flows while avoiding accidental rejection of legitimate callback settlement.
- Make the boundary between “entrypoint gating” and “callback validation” explicit in the FT receiver.

---

## 16. `2d36779` — Gate shadow action entrypoints by contract mode.

This commit aims to bring shadow-action entrypoints under the same maintenance-mode restrictions as other public execution paths.  

### Changes
- In `contracts/contract/src/shadow_actions.rs`, added `assert_operation_allowed_in_current_mode(&account_id)` to:
  - `on_cast_shadow()`;
  - `on_remove_shadow()`.
- These checks now run before processing the Ref Exchange shadow-cast / shadow-remove flows.

### Intent
- Ensure shadow-action entrypoints respect the current contract operating mode instead of remaining as an ungated side path.
- Close a policy gap where maintenance restrictions applied broadly elsewhere but not to these public shadow entry methods.
- Keep incident-mode behavior consistent across execution entry surfaces.

---

## 17. `c285faf` — Update rollback_asset_interest fee and timestamp handling.

This commit aims to make the asset interest rollback tool restore a more complete and internally consistent asset state.  

### Changes
- In `contracts/contract/src/config.rs`:
  - extended `AssetInterestRollbackTarget` to include `prot_fee`;
  - updated `rollback_asset_interest()` so it also rewrites `asset.prot_fee`;
  - stopped taking `last_update_timestamp` from the external rollback target;
  - instead, set `asset.last_update_timestamp = env::block_timestamp()` when rollback is applied.
- Updated the rollback unit tests in `contracts/contract/src/lib.rs` to:
  - provide `prot_fee` in rollback targets;
  - assert that `prot_fee` is restored correctly;
  - assert that `last_update_timestamp` becomes the block time of the rollback transaction.
- Refreshed the liquidation integration test message string to match the current maintenance-mode liquidation error wording.

### Intent
- Include protocol fee state in rollback so the rollback target matches the full economic state implied by the chosen asset snapshot.
- Avoid requiring operators to externally provide a post-rollback timestamp, using the rollback transaction’s block time as the canonical new update time instead.
- Make the rollback interface safer and easier to use during incident repair by reducing operator-controlled fields to the true economic target state.

---

## 18. `e4b8f5f` — Add margin operator close entrypoint.

This commit aims to let a designated margin operator help users initiate `CloseMTPosition` without changing the persisted contract storage layout.

### Changes
- Added `contracts/contract/src/margin_operator.rs`:
  - stores a single margin operator account id through raw `env::storage_read` / `env::storage_write` under `MARGIN_OPERATOR_KEY`;
  - exposes owner-controlled `set_margin_operator()` and view `get_margin_operator()`;
  - adds helper checks for validating the configured operator and ensuring delegated actions are only `CloseMTPosition`.
- Wired the new module into `contracts/contract/src/lib.rs` and added the raw storage key in `contracts/contract/src/storage_keys.rs`.
- In `contracts/contract/src/margin_pyth.rs`, added `margin_operator_close_mtposition_with_pyth()`:
  - requires one yoctoNEAR;
  - checks the predecessor is the configured margin operator;
  - restricts the delegated action to `CloseMTPosition`;
  - executes the close against the specified user margin account through the existing Pyth margin execution flow.
- Added workspace test helpers and extended the Pyth margin trading integration path to cover:
  - rejection before the operator is configured;
  - rejection of non-close delegated margin actions;
  - successful delegated `CloseMTPosition` after setting the operator.

### Intent
- Provide an operational helper role for close-position workflows while keeping the role state outside the main `Contract` storage schema.
- Reuse the existing close-position execution, pricing, pending-action, callback, and settlement paths instead of introducing a parallel close implementation.
- Keep the delegated surface narrow by allowing only `CloseMTPosition`, not arbitrary user margin actions.

---

## 19. `adf7f75` — Gate LP token info sync by contract mode.

This commit applies contract-mode restrictions to the public LP token info sync entrypoint.

### Changes
- In `contracts/contract/src/shadow_actions.rs`, updated `sync_ref_exchange_lp_token_infos()` to:
  - read the caller from `env::predecessor_account_id()`;
  - call `assert_operation_allowed_in_current_mode()` before preparing the Ref Exchange LP token info request.

### Intent
- Prevent unrestricted callers from triggering LP token info cache updates while the contract is in maintenance mode.
- Keep LP-token-related public shadow entrypoints aligned with the existing mode checks on cast/remove shadow flows.

---

## 20. `e64adc9` — Allow asset queries to skip updates.

This commit adds an explicit view option for reading stored asset state without applying the query-time asset update simulation.

### Changes
- In `contracts/contract/src/asset.rs`, extended `get_assets_paged()` with `skip_update: Option<bool>`.
- Preserved the previous default behavior when `skip_update` is omitted or false:
  - assets are still passed through `asset.update()` before being returned.
- Added a raw-read path when `skip_update == Some(true)`:
  - assets are returned directly from storage without calling `asset.update()`.

### Intent
- Let operators inspect the true stored `last_update_timestamp` and related persisted asset fields.
- Preserve compatibility for existing callers by keeping updated asset views as the default response.

---

## 21. `02e7940` — Restrict farm and storage maintenance checks to callers.

This commit aligns maintenance-mode checks for delegated account operations with the intended caller-based access policy.

### Changes
- In `contracts/contract/src/account_farm.rs`, updated `account_farm_claim_all()` so the maintenance-mode whitelist check uses `env::predecessor_account_id()` instead of the optional target `account_id`.
- In `contracts/contract/src/storage.rs`, updated `storage_deposit()` so the maintenance-mode whitelist check uses `env::predecessor_account_id()` instead of the storage target `account_id`.
- Kept the existing target-account behavior unchanged:
  - `account_farm_claim_all(Some(account_id))` still claims farm rewards for the specified account;
  - `storage_deposit(Some(account_id), ...)` still deposits or registers storage for the specified account.

### Intent
- Make maintenance-mode access control consistently restrict who is allowed to call delegated entrypoints, rather than requiring the target account to be whitelisted.
- Allow whitelisted operational accounts to perform farm-claim and storage-deposit actions on behalf of non-whitelisted accounts during maintenance.
- Preserve normal-mode behavior and the existing delegated operation semantics outside the access-control check.

---

## 22. `d9737d4` — Gate staking price sync and beneficiary withdrawal by mode.

This commit closes remaining public user-callable paths that could mutate protocol state without checking the current contract mode.

### Changes
- In `contracts/contract/src/lib.rs`, added a maintenance-mode whitelist check to `sync_staking_token_price()` using `env::predecessor_account_id()`.
- In `contracts/contract/src/config.rs`, added a maintenance-mode whitelist check to `withdraw_beneficiary_fee()` using the beneficiary account, which is derived from `env::predecessor_account_id()`.

### Intent
- Prevent unrestricted callers from triggering staking-token price syncs while the contract is in maintenance mode.
- Make beneficiary fee withdrawals follow the same maintenance-mode access policy as other user withdrawal paths.
- Keep maintenance-mode gating consistent across public non-admin entrypoints that can initiate state changes or token transfers.

---

## Overall Summary

The main thread across this set of commits can be summarized in three parts:

1. **Tightening the risk surface**
   - pre-parse validation for Ref V1 aggregated swap routes;
   - switchable margin actions;
   - maintenance mode;
   - liquidation restricted to whitelisted reliable liquidators during maintenance;
   - a running mode that disables public liquidation while preserving normal operations;
   - LP token info sync, staking-token price sync, and beneficiary fee withdrawal now honor contract-mode restrictions;
   - delegated farm-claim and storage-deposit maintenance checks now restrict the caller rather than the target account.

2. **Adding operational and emergency governance tools**
   - owner-controlled action switches;
   - a maintenance-mode whitelist;
   - an asset interest rollback interface;
   - a one-shot owner withdrawal path;
   - a dedicated pause operator that can move the contract into maintenance.

3. **Fixing fee attribution and execution semantics**
   - holding position fees in liquidation now accrue to `prot_fee`;
   - margin actions now use unified preflight checks;
   - FT-based margin deposit entry flows are now gated by `enabled_actions` through `Deposit`;
   - a comment clarifies the async boundary for `SwapReference` mode checks;
   - `SwapReference` callbacks are now bound to canonical pending-action metadata, including expected DEX, output token, payload fields, and minimum output checks, with suspicious callbacks moving the contract into maintenance and refunding the returned token to the DEX;
   - FT receiver `can_deposit` gating was narrowed so it still applies to true deposit-style entrypoints but no longer blocks margin `SwapReference` business callbacks;
   - shadow action entrypoints now honor contract mode restrictions before processing cast/remove flows;
   - `rollback_asset_interest` now rolls back `prot_fee` and always resets `last_update_timestamp` to the block time of the rollback transaction;
   - a single configured margin operator can now initiate delegated Pyth `CloseMTPosition` flows while the contract stores the role through raw storage outside the main schema;
   - asset pagination views can now optionally skip query-time updates so operators can inspect stored asset timestamps directly.
