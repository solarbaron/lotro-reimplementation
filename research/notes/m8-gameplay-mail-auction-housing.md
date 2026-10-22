<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M8 Gameplay Wire Protocol: Mail System, Auction House, and Player & Kinship Housing

- **Status:** Draft
- **Last updated:** 2026-10-20
- **Author role:** Spec writer (dirty room)
- **Reviewer:** pending

---

## Provenance & Sources <a id="provenance"></a>

| Tag | Method | Source | Details & Question Answered |
| --- | --- | --- | --- |
| `OBS-DAT-3400` | M1 (Install Observation) | `client_highres.dat` / `client_general.dat` (`0x34000000` Master Property Definition, `27,546` properties; `0x23xxxxxx` `EnumMapper`) | Property IDs, property base types (`Int`, `Long64`, `Float`, `Time`, `StringInfo`, `Struct`, `Array`, `Bitfield`, `Enum`), and enum value mappings for Mail (`Mail_*`), Auction House (`Auction_*`), Housing (`HousingSystem_*`, `HouseKeeper_*`, `HousingHook_*`), and Storage/Vault (`Bank_*`, `Vault_*`, `SharedStorage_*`, `StorageLink_*`). |
| `OBS-DAT-3000` | M1 (Install Observation) | `client_general.dat` (`0x30000000` WLib Script Archive, `5,152` classes) | WLib RPC interface classes (`0x0514` Mail, `0x04E5` Auction House, `0x07A7` Housing, `0x169F` Premium/Primary Residence, `0x0A04`/`0x0C20`/`0x0F5C` Housing Chest/Escrow/Vault Storage), client controller classes (`0x0987` -> `0x0173`, `0x0627`, `0x00FA`, `0x07A6`, `0x16A0`, `0x0A02`, `0x0F58`), exact `function_id` values (`(class_id << 16) \| 0x80000000 \| method_id`), and `WLibArgFrame` push/pop sequences (`0x008E:0x0001..0x0009`). M1 was sufficient for all WLib bytecode logic. |
| `BIN-CLIENT` | M4 (Binary Analysis) | `lotroclient64.exe` (v4103.0052.1030.4022, SHA-256 `b522fb5932ea2bcd7ebdc93fb7fb612fb6ebef9fb9bd0bf0e2b918c7b3bd8218`, Capstone x86-64 / Ghidra 11.x) | Native C++ WState serialization (`Serialize`) and deserialization (`Deserialize`) methods for native classes carried inside `WLibArgFrame` object references (`0x008E:0x0004` / `0x008E:0x0009`): `MailLetter` (`0x01B5`), `AuctionQuery` (`0x0188`), `AuctionAdaptor` (`0x0189`), Housing WState classes (`0x07A8`, `0x07A9`, `0x07AF`, `0x07B0`, `0x07B1`, `0x07BC`, `0x07BD`, `0x07C6`, `0x07EF`, `0x07F0`, `0x0801`, `0x0802`, `0x0803`, `0x0804`, `0x16A4`), and Storage/Escrow WState classes (`0x0E9C`, `0x0C1F`, `0x0A07`). M1–M2 were not sufficient because WState class binary layouts are implemented in native C++ rather than WLib bytecode. |

---

## 1. Mail System (`0x0514` RPC Interface & `0x0173` `MailSystem`) <a id="dirty-mail"></a>

### 1.1 Architecture & Transport Framing <a id="dirty-mail-arch"></a>

- **WLib RPC Interface Class:** `0x0514` (`1300`, inherits `0x0264`), mixed into the Player Avatar hierarchy via `0x0987` (`2439`, inherits `0x0173` `MailSystem` and `0x0514`), which is a base class of `0x06C3` (`PlayerAvatar`).
- **World Mailbox Interaction Class:** `0x0627` (`1575`, `MailBoxUseBehavior`, inherits `0x0621` `UseBehavior` and `0x0514`). Checks `Mail_ContainerID` (`0x10000674`) on the world object (`0x0627:v65`) and invokes `0x85140074` (`0x0627:v93`) targeted at the mailbox entity ID.
- **RPC Dispatch Envelope:**
  - All `0x0514` RPCs use **Entity-Targeted RPC dispatch** (`0x00D9:0x000D`, carrying `u64 target_entity_iid`, `u32 function_id`, and `WLibArgFrame`).
  - For `0x85140074`, `target_entity_iid` is the Mailbox world object IID (`0x0627`).
  - For all other C→S and S→C `0x0514` RPCs, `target_entity_iid` is the Player Avatar IID (`0x06C3`).
- **`WLibArgFrame` Serialization Order:**
  - Every `WLibArgFrame` begins with a `u32 arg_byte_size` length prefix, followed by arguments serialized in the exact order of `0x008E:0x0001..0x0005` push calls in the WLib pack helper (and read in the exact order of `0x008E:0x0006..0x0009` pop calls in the S→C unpack stub):
    - `0x008E:0x0001` (pack) / `0x008E:0x0006` (unpack): `u32` (32-bit integer / `PropertyID` / copper coin amount)
    - `0x008E:0x0002` (pack) / `0x008E:0x0007` (unpack): `u32` (32-bit integer / boolean / enum)
    - `0x008E:0x0003` (pack) / `0x008E:0x0008` (unpack): `u64` (64-bit entity or mail instance ID)
    - `0x008E:0x0004` (pack) / `0x008E:0x0009` (unpack): WState object reference (`u32 class_id` or `0x00000000` for null, followed by the class payload)
    - `0x008E:0x0005` (pack) / `0x008E:0x000A` (unpack): `f32` (32-bit IEEE-754 float)

### 1.2 Client-to-Server (C→S) Mail RPCs (`0x0514`) <a id="dirty-mail-c2s"></a>

| `function_id` | Stub / Pack | Caller (`0x0173` / `0x0627`) | Target Entity | `WLibArgFrame` Wire Arguments (in serialization order) | Description |
| --- | --- | --- | --- | --- | --- |
| `0x85140074` | `v20` (`fnum 117`) / `v21` (`0x85140075`) | `0x0627:v93` | Mailbox IID | `u64 entity_iid` (`0x008E:0x0003`) | Interact with world mailbox object to request opening the Mailbox UI. |
| `0x85140048` | `v11` (`fnum 72`) / `v12` (`0x8514004D`) | `0x0173:v6` (`SendMail`) | Player Avatar IID | 1. `u32 is_cod_flag` (`0x008E:0x0002`, `1` = COD, `0` = normal)<br>2. `ref attached_items` (`0x008E:0x0004`, WState `0x0068` `Array<u64>` of up to 10 item IIDs)<br>3. `u32 cod_copper` (`0x008E:0x0001`, copper requested on delivery)<br>4. `u32 enclosed_copper` (`0x008E:0x0001`, copper sent with mail)<br>5. `ref body` (`0x008E:0x0004`, WState `0x00C7` `StringInfo`)<br>6. `ref subject` (`0x008E:0x0004`, WState `0x00C7` `StringInfo`)<br>7. `ref recipient_name` (`0x008E:0x0004`, WState `0x00C7` `StringInfo`)<br>8. `u64 mailbox_iid` (`0x008E:0x0003`) | Send a mail message. Pre-validated by `0x0173:v5` (`CanSendMail`) and postage computed by `0x0173:v28` (`InqPostage` via `0x070D`). |
| `0x85140040` | `v9` (`fnum 64`) / `v13` (`0x8514004E`) | `0x0173:v2` | Player Avatar IID | 1. `u64 mail_id` (`0x008E:0x0003`)<br>2. `u64 mailbox_iid` (`0x008E:0x0003`) | Open / read a mail letter (`mail_id`). Requests full body if not yet populated and marks `Mail_HasBeenOpened` (`0x100001C3`). |
| `0x8514003B` | `v8` (`fnum 59`) / `v11` (`0x8514004C`) | `0x0173:v3` | Player Avatar IID | 1. `u64 mail_id` (`0x008E:0x0003`)<br>2. `u64 mailbox_iid` (`0x008E:0x0003`) | Detach/take all attachments (items and currency; pays COD amount `Mail_CurrencyOnDelivery` `0x10000673` if COD). |
| `0x8514006F` | `v17` (`fnum 111`) / `v18` (`0x85140070`) | `0x0173:v24` | Player Avatar IID | 1. `u64 mail_id` (`0x008E:0x0003`)<br>2. `u64 mailbox_iid` (`0x008E:0x0003`) | Detach item attachment(s) only from `mail_id` (checks `Mail_ItemIID` `0x10000AD7` != 0). |
| `0x85140092` | `v26` (`fnum 146`) / `v27` (`0x85140093`) | `0x0173:v25` | Player Avatar IID | 1. `u64 mail_id` (`0x008E:0x0003`)<br>2. `u64 mailbox_iid` (`0x008E:0x0003`) | Detach currency attachment only (or secondary attachment action) from `mail_id`. |
| `0x85140015` | `v3` (`fnum 21`) / `v15` (`0x85140051`) | `0x0173:v4` | Player Avatar IID | 1. `u64 mail_id` (`0x008E:0x0003`)<br>2. `u64 mailbox_iid` (`0x008E:0x0003`) | Return mail letter `mail_id` to sender (requires `0x01B5:0x001F` `IsReturnable` and `!IsReturned`). |
| `0x85140012` | `v1` (`fnum 18`) / `v14` (`0x85140050`) | `0x0173:v30` | Player Avatar IID | 1. `u64 mail_id` (`0x008E:0x0003`)<br>2. `u64 mailbox_iid` (`0x008E:0x0003`) | Delete mail letter `mail_id` (validated by `0x0173:v31`: cannot delete while `Mail_ItemIID` or `Mail_CurrencyAttachment` remain attached). |

### 1.3 Server-to-Client (S→C) Mail RPCs (`0x0514`) <a id="dirty-mail-s2c"></a>

| `function_id` | Stub | Handler Chain (`0x0987` -> `0x0173` -> UI `0x0148`) | `WLibArgFrame` Wire Arguments (in deserialization order) | Description |
| --- | --- | --- | --- | --- |
| `0x85140025` | `v4` (`fnum 37`) | `0x0987:v39` -> `0x0173:v15` -> `0x0148:0x0006` | `u64 mailbox_iid` (`0x008E:0x0008`) | Open Mailbox UI panel associated with `mailbox_iid`. |
| `0x85140035` | `v6` (`fnum 53`) | `0x0987:v45` -> `0x0173:v1` -> `0x0148:0x0001` | *(empty — 0 bytes)* | Close Mailbox UI panel and clear active `mailbox_iid`. |
| `0x85140088` | `v25` (`fnum 136`) | `0x0987:v43` -> `0x0173:v18` -> `0x0148:0x0007` | `ref inbox_table` (`0x008E:0x0009`, WState `0x00B6` `HashTable<u64, MailLetter(0x01B5)>`) | Deliver full inbox table. Sets `Mail_Count` (`0x100002B0`) on the avatar to the table count, registers item examination tooltips via `0x0173:v27`, and notifies UI `0x0148:0x0007`. |
| `0x8514007F` | `v21` (`fnum 127`) | `0x0987:v42` -> `0x0173:v16` -> `0x0148:0x0009` | `ref mail_letter` (`0x008E:0x0009`, WState `0x01B5` `MailLetter`) | Deliver or update a single `MailLetter` (e.g., new incoming mail or response to `0x85140040` open letter with populated `Mail_Message`). If unread (`0x01B5:0x001C == 0`), increments `Mail_Count` (`0x100002B0`). |
| `0x85140026` | `v5` (`fnum 38`) | `0x0987:v40` -> `0x0173:v19` -> `0x0148:0x0004` | 1. `u64 detached_item_or_flag` (`0x008E:0x0008`)<br>2. `u64 mail_id` (`0x008E:0x0008`) | Confirm attachment(s) / currency detached from `mail_id`; clears COD flag (`0x01B5:0x0022(0)`), clears `Mail_CurrencyOnDelivery`, and updates UI `0x0148:0x0004`. |
| `0x85140036` | `v7` (`fnum 54`) | `0x0987:v38` -> `0x0173:v10` -> `0x0148:0x0002` | `u64 mail_id` (`0x008E:0x0008`) | Unlock / reset pending operation state on `mail_id` in UI (`0x0148:0x0002`). |
| `0x85140081` | `v22` (`fnum 129`) | `0x0987:v46` -> `0x0173:v20` -> `0x0148:0x0008` | `u64 mail_id` (`0x008E:0x0008`) | Confirm `mail_id` deleted/removed from inbox (`0x00B6` table remove); decrements `Mail_Count` (`0x100002B0`) and notifies UI `0x0148:0x0008`. |
| `0x85140083` | `v23` (`fnum 131`) | `0x0987:v41` -> `0x0173:v22` -> `0x0148:0x000A(1)` | *(empty — 0 bytes)* | Confirm outgoing mail sent (`0x0148:0x000A(1)` and `0x0148:0x0003()`). |
| `0x85140086` | `v24` (`fnum 134`) | `0x0987:v44` -> `0x0173:v13` -> `0x0148:0x000A(0)` | `u32 error_pid` (`0x008E:0x0006`) | Mail error response. Displays localized string via `0x041B:v2` and notifies UI `0x0148:0x000A(0)`. |

### 1.4 WState Class `0x01B5` (`437`, `MailLetter`) Wire Format <a id="dirty-mail-wstate"></a>

In `lotroclient64.exe`, WState class `437` (`0x01B5`) has type descriptor `0x1419ab0d0` (vtable `0x141559cd0`):
- `Serialize` / `Deserialize` (`0x140bb15b0` -> `0x140aaa3c0`):
  1. Reads/writes `u32 flags` at `this + 0x00` (4 bytes via `0x1411680d0`).
  2. Reads/writes `PropertySet` (WState class `0x00A6`, `166`) at `this + 0x18` via `0x14043a0f0`.

The 32-bit `flags` bitfield (`this + 0x00`, constants at `.rdata` `0x141a2ff24..0x141a2ff3c`) is manipulated by native accessors `0x01B5:0x0019..0x003A`:

| Bitmask | Bit | Getter (`0x01B5`) | Setter (`0x01B5`) | `.rdata` VA | Meaning |
| --- | --- | --- | --- | --- | --- |
| `0x00000001` | 0 | `0x001C` (`0x140aaa220`) | `0x0019` (`0x140aaa4f0`) | `0x141a2ff24` | `HasBeenOpened` / `IsRead` (checked when counting unread mail in `0x0173:v16`; set by `0x0173:v2` when opening a letter). |
| `0x00000002` | 1 | `0x001D` (`0x140aaa240`) | `0x001A` (`0x140aaa510`) | `0x141a2ff28` | `IsReturned` (letter was returned to sender; checked in `0x0173:v4` & `v9`). |
| `0x00000004` | 2 | `0x0021` (`0x140aaa2d0`) | `0x0020` (`0x140aaa4b0`) | `0x141a2ff2c` | `IsCOD` (Cash-on-Delivery required; cleared when COD attachment is paid/detached in `0x0173:v19`). |
| `0x00000008` | 3 | `0x0024` (`0x140aaa310`) | `0x0025` (`0x140aaa5a0`) | `0x141a2ff30` | `IsAdmin` / `IsSystemMail` (system/GM/auction house automated mail). |
| `0x00000010` | 4 | `0x0032` (`0x140aaa2a0`) | `0x0033` (`0x140aaa410`) | `0x141a2ff34` | `IsSpam` (flagged as spam). |
| `0x00000020` | 5 | `0x0034` (`0x140aaa380`) | `0x0035` (`0x140aaa850`) | `0x141a2ff38` | `IsFaux` / `IsStoreMail` (promotional/store mail). |
| `0x00000040` | 6 | `0x003A` (`0x140aaa2f0`) | `0x0037` (`0x140aaa4d0`) | `0x141a2ff3c` | `CannotReturn` / `NoReturn` (`0x01B5:0x001F` `IsReturnable` at `0x140aaa330` returns `(flags & 0x40) == 0 && IsValidCharacterIID(Mail_SenderIID)`). |

The `PropertySet` (`this + 0x18`, accessed by native methods `0x01B5:0x0001..0x0049`) contains:

| Property ID | Name | Base Type | Accessor in `0x01B5` | Description |
| --- | --- | --- | --- | --- |
| `0x100026F7` | `Mail_MailIID` | `7` (`u64`) | `0x0015` (get) / `0x0016` (set) | Unique 64-bit Mail Letter instance ID (used as key in `0x00B6` inbox hash table). |
| `0x100003A6` | `Mail_ID` | `19` (`u64`) | `0x003C` (get) | Secondary/database 64-bit Mail ID. |
| `0x10001233` | `Mail_SenderIID` | `7` (`u64`) | `0x000B` (get) / `0x000E` (set) | Sender character 64-bit IID. |
| `0x10000F90` | `Mail_SenderName` | `13` (`StringInfo`) | `0x0003` (get) / `0x0007` (set) | Sender character display name. |
| `0x10000925` | `Mail_RecipientIID` | `7` (`u64`) | `0x000A` (get) / `0x000D` (set) | Recipient character 64-bit IID. |
| `0x1000115A` | `Mail_RecipientName` | `13` (`StringInfo`) | `0x0002` (get) / `0x0006` (set) | Recipient character display name. |
| `0x10000926` | `Mail_Subject` | `13` (`StringInfo`) | `0x0004` (get) / `0x0008` (set) | Mail subject line (`<= 1024` chars). |
| `0x10000E8A` | `Mail_Message` | `13` (`StringInfo`) | `0x0001` (get) / `0x0005` (set) | Mail body text (`<= 1024` chars; populated when letter is opened via `0x85140040` -> `0x8514007F`). |
| `0x10001532` | `Mail_MailTimestamp` | `4` (`f64` Time) | `0x0030` (get) / `0x0031` (set) | Mail expiration / delivery timestamp (`f64` seconds). |
| `0x1000057A` | `Mail_CurrencyAttachment` | `15` (`u32`) | `0x000F` (get) / `0x0011` (set) | Enclosed copper coins. |
| `0x10000673` | `Mail_CurrencyOnDelivery` | `15` (`u32`) | `0x0010` (get) / `0x0012` (set) | COD copper coins required to detach items. |
| `0x10000AD7` | `Mail_ItemIID` | `7` (`u64`) | `0x0009` (get) / `0x000C` (set) | Primary attached item 64-bit IID (`0` if none). |
| `0x100010EF` | `UI_Examination_Tooltip_DID` | `11` (`Struct`) | `0x0023` (get) / `0x0027` (set) | Examination tooltip property blob for the primary attached item. |
| `0x10005E77` | `Mail_AdditionalItemAttachmentArray` | `12` (`Array`) | `0x0047`..`0x0049` | Array of `0x10005E76` (`Mail_AdditionalItemAttachmentStruct`, type `11`) for attachment slots 2..10, each containing `Mail_ItemIID` (`0x10000AD7`) and `UI_Examination_Tooltip_DID` (`0x100010EF`). |

---

## 2. Auction House (`0x04E5` RPC Interface, `0x00FA` `AuctionSystem`, `0x00F9` `AuctionControl`) <a id="dirty-auction"></a>

### 2.1 Architecture & Transport Framing <a id="dirty-auction-arch"></a>

- **WLib RPC Interface Class:** `0x04E5` (`1253`, inherits `0x0264`), implemented by `0x00FA` (`250`, `AuctionSystem`), which is a base class of `0x06C3` (`PlayerAvatar`).
- **World Auctioneer Control Class:** `0x00F9` (`249`, `AuctionControl`), carrying fee multipliers (`Auction_PostingFee_FourHours` `0x100016B1`, `EightHours` `0x10001439`, `TwentyFourHours` `0x10000C93`, `FortyEightHours` `0x10000838`, `SeventyTwoHours` `0x100003A0`, `Auction_PostingFee_Minimum` `0x10001537`, `Auction_Kinship_PostingFee_Minimum` `0x10001538`, `Auction_Bid_MinimumIncrease` `0x10001438`).
- **RPC Dispatch Envelope:** All `0x04E5` RPCs use **Entity-Targeted RPC dispatch** (`0x00D9:0x000D`) targeted at the Player Avatar IID (`0x06C3`).
- **Auction House Currency Type (`Enum_AuctionType` `0x2300039C`, property `0x1000507C` `Auction_AuctionType`):**
  - `1`: Coin Auction House (standard copper/silver/gold)
  - `2`: Mithril Coin Auction House (enabled when `World_MithrilCoins_AuctionHouse_Enabled` `0x10005072` is non-zero)

### 2.2 Client-to-Server (C→S) Auction RPCs (`0x04E5`) <a id="dirty-auction-c2s"></a>

| `function_id` | Stub / Pack | Caller (`0x00FA`) | `WLibArgFrame` Wire Arguments (in serialization order) | Description |
| --- | --- | --- | --- | --- |
| `0x84E50018` | `v9` (`fnum 24`) / `v21` (`0x84E50033`) | `0x00FA:v1` (`AddAuction`) | 1. `u64 auctioneer_iid` (`0x008E:0x0003`)<br>2. `ref auction_adaptor` (`0x008E:0x0004`, WState `0x0189` `AuctionAdaptor`) | Post a new auction. Pre-validated by `0x00FA:v30`, `v49`, `v50`: checks item IID (`0x0189:0x000B`), initial price (`0x10000AD1`), optional buyout (`0x10000E84`), duration (`0x100003A3`: `14400` [4h], `28800` [8h], `86400` [24h], or `172800` [48h] seconds), and posting fee (`0x00FA:v3`). |
| `0x84E5001A` | `v11` (`fnum 26`) / `v20` (`0x84E50032`) | `0x00FA:v2` (`BidOnAuction`) | 1. `u64 auctioneer_iid` (`0x008E:0x0003`)<br>2. `u32 bid_or_buyout_amount` (`0x008E:0x0001`)<br>3. `ref auction_adaptor` (`0x008E:0x0004`, WState `0x0189` `AuctionAdaptor`) | Place a bid or immediate buyout (`bid_or_buyout_amount >= Auction_BuyoutPrice`) on an auction listing. Validated by `0x00FA:v48` and `v51`. |
| `0x84E50015` | `v5` (`fnum 21`) / `v19` (`0x84E50031`) | `0x00FA:v4` / `v76` | 1. `u64 auctioneer_iid` (`0x008E:0x0003`)<br>2. `u64 auction_id` (`0x008E:0x0003`) | Cancel a posted auction (`auction_id`). Checked against `Auction_Cancel_Window` (`0x10001BBB`) and `Antispam_AuctionRequest` (`0x10001CB7`). |
| `0x84E50017` | `v7` (`fnum 23`) / `v16` (`0x84E5002E`) | `0x00FA:v5` | 1. `u64 auctioneer_iid` (`0x008E:0x0003`)<br>2. `u64 item_iid` (`0x008E:0x0003`)<br>3. `u64 auction_id` (`0x008E:0x0003`) | Request item examination tooltip data (`0x10000421`) for an auction listing. |
| `0x84E50026` | `v13` (`fnum 38`) / `v22` (`0x84E50034`) | `0x00FA:v6` | 1. `u32 ah_currency_type` (`0x008E:0x0002`, `1`=Coin, `2`=Mithril)<br>2. `u64 auctioneer_iid` (`0x008E:0x0003`)<br>3. `u32 page_number` (`0x008E:0x0001`)<br>4. `u32 sort_descending_flag` (`0x008E:0x0002`)<br>5. `ref auction_query` (`0x008E:0x0004`, WState `0x0188` `AuctionQuery`) | Execute an Auction House search query. |
| `0x84E50003` | `v1` (`fnum 3`) / `v17` (`0x84E5002F`) | `0x00FA:v7` | 1. `u32 ah_currency_type` (`0x008E:0x0002`)<br>2. `u64 auctioneer_iid` (`0x008E:0x0003`) | Request player's own posted auctions list. |
| `0x84E5001D` | `v8` (`fnum 29`) / `v18` (`0x84E50030`) | `0x00FA:v8` | 1. `u32 ah_currency_type` (`0x008E:0x0002`)<br>2. `u64 auctioneer_iid` (`0x008E:0x0003`) | Request player's own active bids list. |
| `0x84E50035` | `v23` (`fnum 53`) / `v25` (`0x84E50037`) | `0x00FA:v54` | 1. `u32 ah_currency_type` (`0x008E:0x0002`)<br>2. `u64 auctioneer_iid` (`0x008E:0x0003`) | Refresh posted auctions list when `Auction_PostListDirty` (`0x10001720`) or `Auction_MCPostListDirty` (`0x1000508C`) is set. |
| `0x84E50036` | `v24` (`fnum 54`) / `v26` (`0x84E50038`) | `0x00FA:v55` | 1. `u32 ah_currency_type` (`0x008E:0x0002`)<br>2. `u64 auctioneer_iid` (`0x008E:0x0003`) | Refresh active bids list when `Auction_BidListDirty` (`0x10001721`) or `Auction_MCBidListDirty` (`0x1000508B`) is set. |
| `0x84E50049` | `v28` (`fnum 73`) / `v29` (`0x84E5004A`) | `0x00FA:v74` | `u32 close_or_mode_flag` (`0x008E:0x0002`) | Notify server of Auction UI panel close / state transition. |

### 2.3 Server-to-Client (S→C) Auction RPCs (`0x04E5`) <a id="dirty-auction-s2c"></a>

| `function_id` | Stub | Handler Chain (`0x00FA` -> UI `0x0133`) | `WLibArgFrame` Wire Arguments (in deserialization order) | Description |
| --- | --- | --- | --- | --- |
| `0x84E5000F` | `v3` (`fnum 15`) | `0x00FA:v43` -> `0x00FA:v19` -> `0x0133:0x0002` | 1. `u32 ah_currency_type` (`0x008E:0x0007`)<br>2. `u64 auctioneer_iid` (`0x008E:0x0008`) | Open Auction House UI panel for `auctioneer_iid`. |
| `0x84E5002C` | `v15` (`fnum 44`) | `0x00FA:v46` -> `0x00FA:v32` -> `0x0133:0x0001` | *(empty — 0 bytes)* | Close Auction House UI panel. |
| `0x84E5001E` | `v10` (`fnum 30`) | `0x00FA:v41` -> `0x00FA:v12` -> `0x0133:0x0006` | 1. `u32 ah_currency_type` (`0x008E:0x0007`)<br>2. `u32 current_page` (`0x008E:0x0006`)<br>3. `u32 total_pages_or_count` (`0x008E:0x0006`)<br>4. `ref results_table` (`0x008E:0x0009`, WState `0x00B6` `HashTable<u64, AuctionAdaptor(0x0189)>`) | Deliver Auction House search results page. |
| `0x84E5000B` | `v2` (`fnum 11`) | `0x00FA:v44` -> `0x00FA:v10` -> `0x0133:0x0007` | 1. `u32 ah_currency_type` (`0x008E:0x0007`)<br>2. `ref posted_table` (`0x008E:0x0009`, WState `0x00B6` `HashTable<u64, AuctionAdaptor(0x0189)>`) | Deliver player's own posted auctions list. |
| `0x84E50014` | `v4` (`fnum 20`) | `0x00FA:v40` -> `0x00FA:v13` -> `0x0133:0x0008` | 1. `u32 ah_currency_type` (`0x008E:0x0007`)<br>2. `ref bids_table` (`0x008E:0x0009`, WState `0x00B6` `HashTable<u64, AuctionAdaptor(0x0189)>`) | Deliver player's own active bids list. |
| `0x84E50021` | `v11` (`fnum 33`) | `0x00FA:v45` -> `0x00FA:v33` -> `0x010C:v10` | `u32 status_or_error_pid` (`0x008E:0x0006`) | Auction operation status or error notification `PropertyID`. |
| `0x84E50024` | `v12` (`fnum 36`) | `0x00FA:v39` -> `0x00FA:v18` -> `0x0133:0x0003` | `ref auction_adaptor` (`0x008E:0x0009`, WState `0x0189`) | Single auction listing added or updated (`0x0133:0x0003`). |
| `0x84E50027` | `v14` (`fnum 39`) | `0x00FA:v42` -> `0x00FA:v28` -> `0x0133:0x0004` | `ref auction_adaptor` (`0x008E:0x0009`, WState `0x0189`) | Single auction listing completed/removed (`0x0133:0x0004`). |
| `0x84E50043` | `v27` (`fnum 67`) | `0x00FA:v63` -> `0x00FA:v26` | `u64 auction_id` (`0x008E:0x0008`) | Auction listing removed/cancelled by `auction_id`. |

### 2.4 WState Classes `0x0188` (`AuctionQuery`) and `0x0189` (`AuctionAdaptor`) <a id="dirty-auction-wstate"></a>

#### 2.4.1 `0x0188` (`392`, `AuctionQuery`)
In `lotroclient64.exe`, WState class `392` (`0x0188`, type descriptor `0x1419aaef0`, vtable `0x1415596a0`) delegates serialization to `0x140bb0eb0` -> `0x1403219f0` (serialize) / `0x140321c00` (deserialize):
1. `u32 sort_column_pid` (`this + 0x10`, accessed by `0x0188:0x0001`/`0x0002`): Property ID of the sort column:
   - `0x100026A5` (`Auction_SortColumn`)
   - `0x100026CD` (`Auction_BidSort`)
   - `0x100026CE` (`Auction_BuyoutSort`)
   - `0x100041CD` (`Auction_BidPerUnitSort`)
   - `0x100041CB` (`Auction_BuyoutPerUnitSort`)
   - `0x100003A2` (`Auction_ItemLevel`)
   - `0x10001CB2` (`Auction_TimeRemaining`)
2. `Array<Property>` (`this + 0x18`, accessed by `0x0188:0x0003`/`0x0004`): Serialized with the standard WState array header (`u32 capacity_and_flags`, `u32 count`), followed by `count` serialized `Property` (`0x0027` via `0x140439770`) entries representing the search filters constructed by `0x00FA`:
   - `0x10001533` (`Auction_CategoryType`, type `8` `u32` enum `0x230001D1` `Enum_AuctionCategoryType`)
   - `0x10000D95` (`Auction_LevelRange`, type `11` struct containing `0x10001155` `Auction_MinLevel` and `0x100001C2` `Auction_MaxLevel`)
   - `0x10000F8B` (`Auction_StringSearch`, type `11` struct containing `0x10001065` `Auction_ItemName`, `0x10000A08` `Auction_SearchLanguage`, and `0x10001594` `Auction_ExclusiveSearch`)
   - `0x10001593` (`Auction_KinshipSearch`, type `11` struct containing `0x10000BAA` `Auction_KinshipID`)
   - `0x10001BB6` (`Auction_SearchableProperty_Set`, type `11` struct containing item quality, `0x10000AD2` `Auction_EquipmentCategory`, and `0x1000263B` `Auction_ClassModSearch`)
   - `0x10002684` (`Auction_IASearch`, type `11` struct for Legendary Item filter: `0x10002683` `Auction_IAStars`, `0x10002669` `Auction_IAProgressionColumn`)

#### 2.4.2 `0x0189` (`393`, `AuctionAdaptor`)
In `lotroclient64.exe`, WState class `393` (`0x0189`, type descriptor `0x1419aaf40`, vtable `0x141559738`) delegates serialization to `0x140bb0f00` -> `0x140a48500` (serialize) / `0x140a48540` (deserialize):
1. `u64 item_iid` (`this + 0x10`, via `[rax + 0x30]`, accessed by `0x0189:0x000B`/`0x0018`): 64-bit instance ID of the auctioned item.
2. `PropertySet` (`this + 0x18`, WState class `0x00A6` via `0x14043a040` / `0x14043a0f0`), containing:
   - `0x10000002` (`Auction_ID`, type `7` `u64`, `0x0189:0x0001`/`0x0016`)
   - `0x1000507C` (`Auction_AuctionType`, type `8` `u32` enum `0x2300039C`, `1`=Coin, `2`=Mithril)
   - `0x10000AD0` (`Auction_Poster`, type `7` `u64`, `0x0189:0x0009`/`0x001C`)
   - `0x10000A07` (`Auction_PosterName`, type `13` `StringInfo`, `0x0189:0x000E`/`0x001B`)
   - `0x10000BAA` (`Auction_KinshipID`, type `7` `u64`, `0x0189:0x000C`, `0` if public)
   - `0x1000074C` (`Auction_KinshipName`, type `13` `StringInfo`, `0x0189:0x000D`)
   - `0x10000AD1` (`Auction_InitialPrice`, type `15` `u32`, `0x0189:0x0008`/`0x0013`)
   - `0x10000E84` (`Auction_BuyoutPrice`, type `15` `u32`, `0x0189:0x0006`/`0x0012`)
   - `0x10000D96` (`Auction_CurrentBid`, type `15` `u32`, `0x0189:0x0003`/`0x0010`)
   - `0x10000F8C` (`Auction_CurrentBidder`, type `7` `u64`, `0x0189:0x0004`/`0x0017`)
   - `0x100002AC` (`Auction_AuctionBids`, type `15` `u32`, `0x0189:0x0002`/`0x000F`)
   - `0x100003A3` (`Auction_Duration`, type `15` `u32`, `0x0189:0x0005`/`0x0011`, duration in seconds)
   - `0x100003A2` (`Auction_ItemLevel`, type `15` `u32`)
   - `0x100041CC` (`Auction_ItemQuantity`, type `15` `u32`)
   - `0x10000D94` (`Auction_TimeReceived`, type `4` `f64`, `0x0189:0x000A`/`0x0014`)
3. `Property` (`this + 0x78`, WState class `0x0027` via `0x140439700` / `0x140439770`, accessed by `0x0189:0x0007`/`0x0015`): `0x10000421` (`UI_Examination_Tooltip_DID`) carrying the item tooltip examination data.

---

## 3. Player & Kinship Housing (`0x07A7`, `0x169F`, `0x0A04`, `0x0C20`, `0x0F5C`) <a id="dirty-housing"></a>

### 3.1 Architecture, Transport Framing & Packed `house_id` <a id="dirty-housing-arch"></a>

- **Primary Housing RPC Interface:** `0x07A7` (`1959`, inherits `0x0264`), implemented by `0x07A6` (`1958`), which is a base class of `0x06C3` (`PlayerAvatar`).
  - **Transport:** All `0x07A7` RPCs use **Broadcast / Non-Entity-Targeted RPC dispatch** (`0x00D9:0x000C`, carrying `u32 function_id` and `WLibArgFrame` without a target entity IID).
- **Premium Housing & Primary Residence RPC Interface:** `0x169F` (`5791`, inherits `0x0264`), implemented by `0x16A0` (`5792`), a base class of `0x06C3` (`PlayerAvatar`).
  - **Transport:** Entity-targeted RPC dispatch (`0x00D9:0x000D`) on the Player Avatar IID (`0x06C3`).
- **Housing Chest, Escrow, and Vault Storage RPC Interfaces:**
  - `0x0A04` (`2564`, implemented by `0x0A02` on `0x06C3`): Broadcast RPC dispatch (`0x00D9:0x000C`) using `0x0E9C` (`BankAddress`) to address Personal Vault, Shared Storage, Kinship Vault, Housing Chest, and Housing Escrow containers.
  - `0x0C20` (`3104`): Broadcast RPC dispatch (`0x00D9:0x000C`) for chest capacity/tab configuration (`0x8C200005`).
  - `0x0F5C` (`3932`, implemented by `0x0F58` on `0x06C3`): Entity-targeted RPC dispatch (`0x00D9:0x000D`) for `StorageLink` (`0x100041C9` `StorageLink_LinkType`) containers.
- **Packed 64-Bit `house_id` Representation (`0x07A1:v14`, `v17`, `v18`):**
  - Across all `0x07A7` RPCs and housing WState structs, a 64-bit `house_id` packs the 32-bit Neighborhood DID (`0x10001A49` `HouseKeeper_NeighborhoodDID`) into the upper 32 bits and the 32-bit House DID (`0x1000190A` `HouseKeeper_HouseDID`) into the lower 32 bits:
    - `house_id = (u64(neighborhood_did) << 32) | u64(house_did)`
    - `neighborhood_did = u32(house_id >> 32)` (`0x07A1:v17`)
    - `house_did = u32(house_id & 0xFFFFFFFF)` (`0x07A1:v18`)

### 3.2 Client-to-Server (C→S) Housing RPCs (`0x07A7` & `0x169F`) <a id="dirty-housing-c2s"></a>

| `function_id` | Stub / Pack | Caller (`0x07A6` / `0x16A0`) | `WLibArgFrame` Wire Arguments (in serialization order) | Description |
| --- | --- | --- | --- | --- |
| `0x87A70002` | `v2` (`fnum 2`) / `v3` (`0x87A70003`) | `0x07A6:v139` | 1. `u32 house_did` (`0x008E:0x0002`)<br>2. `u32 neighborhood_did` (`0x008E:0x0002`)<br>3. `u64 gate_iid` (`0x008E:0x0003`) | Enter / visit a neighborhood or specific house via neighborhood gate (`gate_iid`). |
| `0x87A7000B` | `v10` (`fnum 11`) / `v9` (`0x87A70033`) | `0x07A6:v140` | 1. `u32 writ_or_alt_currency_flag` (`0x008E:0x0002`)<br>2. `u32 house_did` (`0x008E:0x0002`)<br>3. `u32 neighborhood_did` (`0x008E:0x0002`)<br>4. `u32 template_did` (`0x008E:0x0002`)<br>5. `u32 vendor_did` (`0x008E:0x0002`)<br>6. `u64 vendor_iid` (`0x008E:0x0003`) | Purchase a house with coin or currency (`0x07A6:v140`; Note: when purchasing via Mithril/Writ paper item `fp-6 != 0`, client also invokes PaperItemService `0x0F44:0x0003` service `29` `0x001D` with `0x10005E36`, `0x10005E34`, `0x10005E35`, `0x10001A49`). |
| `0x87A7000A` | `v8` (`fnum 10`) / `v11` (`0x87A70034`) | `0x07A6:v141` | 1. `u32 action_flag` (`0x008E:0x0002`)<br>2. `u32 house_did` (`0x008E:0x0002`)<br>3. `u32 neighborhood_did` (`0x008E:0x0002`)<br>4. `u64 vendor_iid` (`0x008E:0x0003`) | Secondary house purchase / transfer action via vendor. |
| `0x87A70008` | `v6` (`fnum 8`) / `v7` (`0x87A70030`) | `0x07A6:v143` | `u32 template_did` (`0x008E:0x0002`) | Request neighborhood directory list (`0x07B1`) for homestead `template_did`. |
| `0x87A7003B` | `v37` (`fnum 59`) / `v40` (`0x87A700BD`) | `0x07A6:v180` | `u32 neighborhood_did` (`0x008E:0x0002`) | Request list of houses (`0x07A9`) in `neighborhood_did`. |
| `0x87A7003C` | `v34` (`fnum 60`) / `v41` (`0x87A700BE`) | `0x07A6:v181` | `u32 template_did` (`0x008E:0x0002`) | Request available houses for sale in homestead `template_did`. |
| `0x87A70040` | `v43` (`fnum 64`) / `v44` (`0x87A70103`) | `0x07A6:v185` | `u32 neighborhood_did` (`0x008E:0x0002`) | Request detailed house purchase listings (`0x0803`) for `neighborhood_did`. |
| `0x87A7001E` | `v30` (`fnum 30`) / `v31` (`0x87A7008D`) | `0x07A6:v174` | *(empty — 0 bytes)* | Request player's own personal/kinship house information. |
| `0x87A70022` | `v32` (`fnum 34`) / `v33` (`0x87A70091`) | `0x07A6:v176` | `u64 house_id` (`0x008E:0x0003`) | Abandon / relinquish owned house (`house_id`). |
| `0x87A70006` | `v14` (`fnum 6`) / `v15` (`0x87A70046`) | `0x07A6:v147` | 1. `u32 house_did` (`0x008E:0x0002`)<br>2. `u32 neighborhood_did` (`0x008E:0x0002`)<br>3. `u64 housekeeper_iid` (`0x008E:0x0003`) | Request housing permissions (`0x07BC`) for a house. |
| `0x87A70019` | `v23` (`fnum 25`) / `v25` (`0x87A70069`) | `0x07A6:v156` | 1. `u32 house_did` (`0x008E:0x0002`)<br>2. `u32 neighborhood_did` (`0x008E:0x0002`)<br>3. `u64 housekeeper_iid` (`0x008E:0x0003`) | Request housing upkeep/maintenance info (`0x07C6`) for a house. |
| `0x87A7002D` | `v26` (`fnum 45`) / `v29` (`0x87A7007E`) | `0x07A6:v166` | 1. `u32 upkeep_amount_or_credits` (`0x008E:0x0001`)<br>2. `u64 house_id` (`0x008E:0x0003`) | Pay house upkeep/maintenance with standard coin. |
| `0x87A701DE` | `v59` (`fnum 478`) / `v60` (`0x87A701DF`) | `0x07A6:v229` | 1. `u32 upkeep_periods` (`0x008E:0x0001`)<br>2. `u64 house_id` (`0x008E:0x0003`) | Pay house upkeep/maintenance with Mithril Coins. |
| `0x87A70016` | `v18` (`fnum 22`) / `v20` (`0x87A70054`) | `0x07A6:v153` | 1. `u32 permission_bitmask` (`0x008E:0x0001`)<br>2. `u32 permission_group_index` (`0x008E:0x0001`)<br>3. `u32 house_did` (`0x008E:0x0002`)<br>4. `u64 housekeeper_iid` (`0x008E:0x0003`) | Update housing permission bitmask for a permission group/class (`0x100019B0` `HousingSystem_Permissions`). |
| `0x87A7003A` | `v21` (`fnum 58`) / `v38` (`0x87A700B4`) | `0x07A6:v180` | `u64 target_char_iid` (`0x008E:0x0003`) | Request character display name resolution for a permission entry IID (`0x87A70177`). |
| `0x87A70071` | `v22` (`fnum 113`) / `v27` (`0x87A70072`) | `0x07A6:v162` | 1. `u32 permission_bitmask` (`0x008E:0x0001`)<br>2. `u64 target_char_iid` (`0x008E:0x0003`)<br>3. `u64 house_id` (`0x008E:0x0003`) | Add an individual character (`target_char_iid`) to the house guest permission list. |
| `0x87A70077` | `v47` (`fnum 119`) / `v28` (`0x87A70078`) | `0x07A6:v205` | 1. `ref target_char_name` (`0x008E:0x0004`, WState `0x00C7` `StringInfo`)<br>2. `u64 house_id` (`0x008E:0x0003`) | Add an individual character by display name to the house guest permission list. |
| `0x87A700A5` | `v49` (`fnum 165`) / `v35` (`0x87A700A6`) | `0x07A6:v206` | 1. `u64 target_char_iid` (`0x008E:0x0003`)<br>2. `u64 house_id` (`0x008E:0x0003`) | Remove an individual character (`target_char_iid`) from the house guest permission list. |
| `0x87A70116` | `v57` (`fnum 278`) / `v45` (`0x87A70117`) | `0x07A6:v203` | 1. `u32 permission_bitmask` (`0x008E:0x0001`)<br>2. `u64 target_char_iid` (`0x008E:0x0003`)<br>3. `u64 house_id` (`0x008E:0x0003`) | Update permission bitmask for an existing guest character (`target_char_iid`). |
| `0x87A70138` | `v58` (`fnum 312`) / `v46` (`0x87A70139`) | `0x07A6:v210` | 1. `u32 panel_type_flag` (`0x008E:0x0001`)<br>2. `u64 house_id` (`0x008E:0x0003`) | Notify server of closing housing permissions/upkeep UI panel. |
| `0x87A701B1` | `v61` (`fnum 433`) / `v52` (`0x87A701B2`) | `0x07A6:v216` | 1. `u32 enable_flag` (`0x008E:0x0001`)<br>2. `u64 housekeeper_iid` (`0x008E:0x0003`) | Toggle Housing Decoration Mode (`HousingSystem_PlayerInDecorationMode` `0x100019B2`). |
| `0x87A701B5` | `v63` (`fnum 437`) / `v53` (`0x87A701B6`) | `0x07A6:v218` | 1. `u32 hook_id` (`0x008E:0x0002`)<br>2. `u64 item_iid` (`0x008E:0x0003`)<br>3. `u64 housekeeper_iid` (`0x008E:0x0003`) | Place / slot a decoration item (`item_iid`) into decoration hook `hook_id` (`HousingHook_LandmarkHookID` `0x100019FD`). |
| `0x87A701B7` | `v64` (`fnum 439`) / `v54` (`0x87A701B8`) | `0x07A6:v219` | 1. `u32 hook_id` (`0x008E:0x0002`)<br>2. `u64 housekeeper_iid` (`0x008E:0x0003`) | Remove / unslot the decoration item from decoration hook `hook_id`. |
| `0x87A701BF` | `v66` (`fnum 447`) / `v56` (`0x87A701C0`) | `0x07A6:v221` | 1. `f32 heading_degrees` (`0x008E:0x0005`)<br>2. `u32 hook_id` (`0x008E:0x0002`)<br>3. `u64 housekeeper_iid` (`0x008E:0x0003`) | Rotate the decoration item in `hook_id` to `heading_degrees`. |
| `0x87A701F4` | `v68` (`fnum 500`) / `v67` (`0x87A701F5`) | `0x07A6:v240` | 1. `ref offset_vector` (`0x008E:0x0004`, WState `0x00D3` `Vector` + rotation)<br>2. `u64 housekeeper_iid` (`0x008E:0x0003`)<br>3. `u32 hook_id` (`0x008E:0x0002`) | Adjust 3D translation offset and rotation of the decoration in `hook_id` (`HousingHook_TranslationOffsetVector` `0x10005A05`). |
| `0x87A701F0` | `v69` (`fnum 496`) / `v65` (`0x87A701F1`) | `0x07A6:v233` | `u64 housekeeper_iid` (`0x008E:0x0003`) | Reset / commit decoration hook state on `housekeeper_iid`. |
| `0x87A70210` | `v70` (`fnum 528`) / `v72` (`0x87A70211`) | `0x07A6:v249` | `u64 housekeeper_iid` (`0x008E:0x0003`) | Reset ambient house music / dayfile hook 1 (`0x07A6:v249`). |
| `0x87A70209` | `v71` (`fnum 521`) / `v73` (`0x87A70212`) | `0x07A6:v248` | `u64 housekeeper_iid` (`0x008E:0x0003`) | Reset ambient house music / dayfile hook 2 (`0x07A6:v248`). |
| `0x969F000C` | `v1` (`fnum 12`) / `v2` (`0x969F000D`) | `0x16A0:v1` | `u32 query_flag` (`0x008E:0x0002`) | Query primary residence / premium housing status (targeted at Player Avatar IID). |
| `0x969F0018` | `v4` (`fnum 24`) / `v5` (`0x969F0019`) | `0x16A0:v2` | 1. `u64 target_house_address` (`0x008E:0x0003`)<br>2. `u64 secondary_id` (`0x008E:0x0003`) | Set primary residence / premium house target address (`0x100068AD` `HousingSystem_PrimaryResidenceTargetAddress`). |
| `0x969F004C` | `v9` (`fnum 76`) / `v10` (`0x969F004D`) | `0x16A0:v16` | 1. `u32 arg0` (`0x008E:0x0002`)<br>2. `u32 arg1` (`0x008E:0x0002`) | Secondary premium housing action. |

### 3.3 Server-to-Client (S→C) Housing RPCs (`0x07A7` & `0x169F`) <a id="dirty-housing-s2c"></a>

| `function_id` | Stub | Handler Chain (`0x07A6` / `0x16A0`) | `WLibArgFrame` Wire Arguments (in deserialization order) | Description |
| --- | --- | --- | --- | --- |
| `0x87A70001` | `v1` (`fnum 1`) | `0x07A6:v138` -> `0x0143:0x0011` | 1. `u32 template_did` (`0x008E:0x0007`)<br>2. `u64 gate_iid` (`0x008E:0x0008`) | Open Neighborhood Gate UI (`0x0143:0x0011`). |
| `0x87A70007` | `v4` (`fnum 7`) | `0x07A6:v142` -> `0x0143:0x0012` | 1. `u32 template_did` (`0x008E:0x0007`)<br>2. `u64 vendor_iid` (`0x008E:0x0008`) | Open House Broker / Purchase Vendor UI (`0x0143:0x0012`). |
| `0x87A70012` | `v5` (`fnum 18`) | `0x07A6:v144` -> `0x0143:0x0013` | 1. `ref neighborhood_list` (`0x008E:0x0009`, WState `0x07B1`)<br>2. `u32 status` (`0x008E:0x0006`) | Deliver neighborhood directory list (`0x07B1` containing `Array<0x07AF>`). |
| `0x87A7002A` | `v12` (`fnum 42`) | `0x07A6:v145` -> `0x0143:0x0014` | 1. `u32 status` (`0x008E:0x0006`)<br>2. `u32 house_or_neighborhood_did` (`0x008E:0x0007`) | House purchase / gate entry status response. |
| `0x87A70042` | `v13` (`fnum 66`) | `0x07A6:v148` -> `0x0143:0x0015` | 1. `ref permissions` (`0x008E:0x0009`, WState `0x07BC` `HousingPermissions`)<br>2. `u32 house_did` (`0x008E:0x0007`)<br>3. `u32 neighborhood_did` (`0x008E:0x0007`) | Deliver housing permissions (`0x07BC`). |
| `0x87A70049` | `v16` (`fnum 73`) | `0x07A6:v150` -> `0x0143:0x0016` | 1. `u32 status` (`0x008E:0x0006`)<br>2. `u32 arg1` (`0x008E:0x0006`) | Group permission update status response. |
| `0x87A7004E` | `v17` (`fnum 78`) | `0x07A6:v151` -> `0x0143:0x0017` | 1. `u32 status` (`0x008E:0x0006`)<br>2. `u64 target_char_iid` (`0x008E:0x0008`) | Individual guest permission update status response. |
| `0x87A70051` | `v19` (`fnum 81`) | `0x07A6:v154` -> `0x0143:0x0018` | 1. `u32 status` (`0x008E:0x0006`)<br>2. `u32 house_did` (`0x008E:0x0007`)<br>3. `u32 neighborhood_did` (`0x008E:0x0007`) | Upkeep payment / permission status response. |
| `0x87A70062` | `v24` (`fnum 98`) | `0x07A6:v157` -> `0x0143:0x001A` | 1. `ref upkeep_info` (`0x008E:0x0009`, WState `0x07C6` `HousingUpkeepInfo`)<br>2. `u32 house_did` (`0x008E:0x0007`)<br>3. `u32 neighborhood_did` (`0x008E:0x0007`) | Deliver house upkeep/maintenance info (`0x07C6`). |
| `0x87A700AF` | `v36` (`fnum 175`) | `0x07A6:v179` -> `0x0143:0x001F` | 1. `u64 status_or_owner_iid` (`0x008E:0x0008`)<br>2. `u64 house_id` (`0x008E:0x0008`) | House ownership / abandon confirmation. |
| `0x87A700B9` | `v39` (`fnum 185`) | `0x07A6:v183` -> `0x0143:0x0020` | 1. `ref house_list` (`0x008E:0x0009`, WState `0x07A9`)<br>2. `u32 status` (`0x008E:0x0006`) | Deliver list of houses in a neighborhood (`0x07A9` containing `Array<0x07A8>`). |
| `0x87A700C2` | `v42` (`fnum 194`) | `0x07A6:v186` -> `0x0143:0x0022` | 1. `ref purchase_list` (`0x008E:0x0009`, WState `0x0803`)<br>2. `u32 status` (`0x008E:0x0006`) | Deliver available houses for sale (`0x0803` containing `Array<0x0801>`). |
| `0x87A70174` | `v48` (`fnum 372`) | `0x07A6:v212` -> `0x0143:0x0019` | 1. `u32 arg0` (`0x008E:0x0006`)<br>2. `u32 arg1` (`0x008E:0x0006`) | Housing decoration/upkeep callback. |
| `0x87A70177` | `v50` (`fnum 375`) | `0x07A6:v214` -> `0x0143:0x0021` | 1. `u64 char_iid` (`0x008E:0x0008`)<br>2. `ref char_name` (`0x008E:0x0009`, WState `0x00C7` `StringInfo`) | Deliver resolved character display name for housing permission list. |
| `0x87A701AE` | `v51` (`fnum 430`) | `0x07A6:v215` -> `0x0143:0x001C` | 1. `u32 hook_id` (`0x008E:0x0007`)<br>2. `u64 item_iid` (`0x008E:0x0008`)<br>3. `u32 status` (`0x008E:0x0006`) | Decoration hook slot/unslot result. |
| `0x87A701BB` | `v55` (`fnum 443`) | `0x07A6:v220` -> `0x0143:0x001D` | 1. `u32 hook_id` (`0x008E:0x0007`)<br>2. `f32 heading_degrees` (`0x008E:0x000A`)<br>3. `u32 status` (`0x008E:0x0006`) | Decoration hook rotation result. |
| `0x87A701E4` | `v62` (`fnum 484`) | `0x07A6:v230` -> `0x0131:0x0001` | `u32 mithril_upkeep_status` (`0x008E:0x0006`) | Mithril Coin upkeep payment status response. |
| `0x969F0016` | `v3` (`fnum 22`) | `0x16A0:v6` | `u64 house_address` (`0x008E:0x0008`) | Primary residence / premium house address update. |
| `0x969F001E` | `v6` (`fnum 30`) | `0x16A0:v13` | `u32 status` (`0x008E:0x0006`) | Primary residence status callback 1. |
| `0x969F0020` | `v7` (`fnum 32`) | `0x16A0:v12` | `u32 status` (`0x008E:0x0006`) | Primary residence status callback 2. |
| `0x969F0023` | `v8` (`fnum 35`) | `0x16A0:v11` | `u32 status` (`0x008E:0x0006`) | Primary residence status callback 3. |

### 3.4 Housing Chest, Vault & Escrow Storage RPCs (`0x0A04`, `0x0C20`, `0x0F5C`) <a id="dirty-housing-storage"></a>

Housing Chests and Escrow brokers share the unified Bank/Vault RPC protocol (`0x0A04`, `2564`, implemented by `0x0A02` on `0x06C3`), discriminated by WState class `0x0E9C` (`3740`, `BankAddress`, 16 bytes: `u64 owner_or_house_iid`, `u32 bank_repository_type`, `u32 chest_type`).

| `function_id` | Direction | Transport | `WLibArgFrame` Wire Arguments (in serialization order) | Description |
| --- | --- | --- | --- | --- |
| `0x8A040004` | C→S (`v1`, pack `v3`/`0x8A040008`) | Broadcast (`0x00D9:0x000C`) | 1. `u64 chest_or_npc_iid` (`0x008E:0x0003`)<br>2. `ref bank_address` (`0x008E:0x0004`, WState `0x0E9C` `BankAddress`) | Request to open a Housing Chest, Escrow, Kinship Vault, or Personal Vault container (`0x0A02:v131`). |
| `0x8A040007` | C→S (`v2`, pack `v4`/`0x8A040009`) | Broadcast (`0x00D9:0x000C`) | `ref bank_address` (`0x008E:0x0004`, WState `0x0E9C`) | Close the open storage container (`0x0A02:v132`). |
| `0x8A040014` | C→S (`v7`, pack `v10`/`0x8A040019`) | Broadcast (`0x00D9:0x000C`) | 1. `u32 quantity` (`0x008E:0x0001`)<br>2. `u32 dest_chest_type` (`0x008E:0x0002`, `Enum_ChestType` `0x230001CD`)<br>3. `u64 item_iid` (`0x008E:0x0003`)<br>4. `ref bank_address` (`0x008E:0x0004`, WState `0x0E9C`) | Deposit item (`item_iid`) into storage chest/vault (`0x0A02:v135`). |
| `0x8A040017` | C→S (`v8`, pack `v11`/`0x8A04001A`) | Broadcast (`0x00D9:0x000C`) | 1. `u32 quantity` (`0x008E:0x0001`)<br>2. `u64 dest_container_iid` (`0x008E:0x0003`, e.g. player backpack IID)<br>3. `u64 item_iid` (`0x008E:0x0003`)<br>4. `ref bank_address` (`0x008E:0x0004`, WState `0x0E9C`) | Withdraw item (`item_iid`) from Housing Chest, Escrow, or Vault into `dest_container_iid` (`0x0A02:v136`). |
| `0x8A040018` | C→S (`v9`, pack `v12`/`0x8A040021`) | Broadcast (`0x00D9:0x000C`) | 1. `u32 dest_chest_type` (`0x008E:0x0002`)<br>2. `u64 item_iid` (`0x008E:0x0003`)<br>3. `ref bank_address` (`0x008E:0x0004`, WState `0x0E9C`) | Move item (`item_iid`) to another chest tab (`dest_chest_type`) within `bank_address` (`0x0A02:v137`). |
| `0x8A04000C` | S→C (`v5`) | Broadcast (`0x00D9:0x000C`) | 1. `ref bank_address` (`0x008E:0x0009`, WState `0x0E9C`)<br>2. `ref items_array` (`0x008E:0x0009`, WState `0x0022` `Array<VaultItem(0x0A07)>`)<br>3. `ref chest_info_array` (`0x008E:0x0009`, WState `0x0022` `Array<BankChestInfo(0x0C1F)>`)<br>4. `u32 max_capacity` (`0x008E:0x0006`)<br>5. `u32 used_capacity_or_flags` (`0x008E:0x0006`)<br>6. `u64 npc_or_chest_iid` (`0x008E:0x0008`) | Open storage UI and deliver full chest/escrow item list (`0x0A02:v133`). |
| `0x8A040010` | S→C (`v6`) | Broadcast (`0x00D9:0x000C`) | 1. `ref bank_address` (`0x008E:0x0009`, WState `0x0E9C`)<br>2. `u32 error_or_close_pid` (`0x008E:0x0006`) | Close storage container or report error (`0x0A02:v134`). |
| `0x8A04001B` | S→C (`v13`) | Broadcast (`0x00D9:0x000C`) | 1. `ref bank_address` (`0x008E:0x0009`, WState `0x0E9C`)<br>2. `ref vault_item` (`0x008E:0x0009`, WState `0x0A07`)<br>3. `u32 update_flag` (`0x008E:0x0006`) | Single item added or updated in storage (`0x0A02:v138`). |
| `0x8A040020` | S→C (`v14`) | Broadcast (`0x00D9:0x000C`) | 1. `ref bank_address` (`0x008E:0x0009`, WState `0x0E9C`)<br>2. `u32 chest_type` (`0x008E:0x0007`)<br>3. `u32 arg2` (`0x008E:0x0006`)<br>4. `u32 arg3` (`0x008E:0x0006`)<br>5. `u32 arg4` (`0x008E:0x0006`)<br>6. `u64 item_iid` (`0x008E:0x0008`) | Item removed or moved from storage (`0x0A02:v139`). |

### 3.5 Native C++ Housing & Storage WState Classes <a id="dirty-housing-wstate"></a>

Verified from `lotroclient64.exe` native `Serialize` / `Deserialize` implementations:

| Class ID | Name | Native Serialize / Deserialize | Wire Layout Summary |
| --- | --- | --- | --- |
| `0x07A8` (`1960`) | `NeighborhoodHouseInfo` | `0x140bb1c90` -> `0x140ab1280` / `0x140ab1390` | 1. `u64 house_id` (`+0x10`)<br>2. `u32 house_template_did` (`+0x18`)<br>3. `StringInfo owner_name` (`+0x20`, via `0x140343fb0`)<br>4. `u32 price` (`+0x60`)<br>5. `u32 upkeep_cost` (`+0x64`)<br>6. `u8 flags_or_type` (`+0x68`)<br>7. `u64 owner_iid` (`+0x70`)<br>8. `StringInfo kinship_or_secondary_name` (`+0x78`)<br>9. `u8 is_kinship_or_locked` (`+0xB8`) |
| `0x07A9` (`1961`) | `NeighborhoodHouseList` | `0x140bb1ce0` -> `0x14032c190` / `0x14032c8c0` | 1. `u32 neighborhood_did` (`+0x08`)<br>2. `u32 template_did` (`+0x0C`)<br>3. `Array<0x07A8>` (`+0x10`: `2 × u32` array header + `N` serialized `0x07A8` entries) |
| `0x07AF` (`1967`) | `NeighborhoodInstanceInfo` | `0x140bb1d30` -> `0x140ab1980` / `0x140ab1a50` | 1. `u32 neighborhood_did` (`+0x10`)<br>2. `u32 neighborhood_name_token_or_id` (`+0x14`)<br>3. `u32 template_did` (`+0x18`)<br>4. `u8 status_flags` (`+0x1C`)<br>5. `Array<u32>` (`+0x20`: `2 × u32` header + `N × u32` available house counts by type)<br>6. `Array<u64>` (`+0x30`: `2 × u32` header + `N × u64` friend/kinship house IDs) |
| `0x07B1` (`1969`) | `NeighborhoodDirectoryList` | `0x140bb1dd0` -> `0x14032c200` / `0x14032c9b0` | 1. `u32 template_did` (`+0x08`)<br>2. `Array<0x07AF>` (`+0x10`: `2 × u32` header + `N` serialized `0x07AF` entries) |
| `0x07BC` (`1980`) | `HousingPermissions` | `0x140bb1e70` -> `0x14032c2f0` / `0x14032cb90` | 1. `10 × u32` group permission bitmasks (`+0x08..+0x2C`, 40 bytes)<br>2. `HashTable<u64, u32>` (`+0x30`: `u32 flags`, `u32 bucket_idx`, `u32 count`, followed by `count` pairs of `[u64 char_iid, u32 permission_bitmask]`)<br>3. `u64 owner_iid` (`+0x50`)<br>4. `u8 is_anonymous_or_flag` (`+0x58`) |
| `0x07BD` (`1981`) | `HousingDecorationInfo` | `0x140bb1ec0` -> `0x140ab0da0` / `0x140ab0ef0` | 1. `u32 item_did` (`+0x10`)<br>2. `u64 item_iid` (`+0x18`)<br>3. `u32 hook_id` (`+0x20`)<br>4. `u32 hook_category` (`+0x24`)<br>5. `f32 heading_degrees` (`+0x28`)<br>6. `StringInfo item_name` (`+0x30`)<br>7. `u64 owner_iid` (`+0x70`)<br>8. `StringInfo owner_name` (`+0x78`)<br>9. `u8 is_bound_flag` (`+0xB8`)<br>10. `u8 return_to_owner_flag` (`+0xB9`) |
| `0x07C6` (`1990`) | `HousingUpkeepInfo` | `0x140bb1f10` -> `0x140ab2470` / `0x140ab2520` | 1. `u32 upkeep_base_cost` (`+0x10`)<br>2. `u32 upkeep_adjusted_cost` (`+0x14`)<br>3. `u32 max_upkeep_periods` (`+0x18`)<br>4. `f64 next_upkeep_due_time` (`+0x20`)<br>5. `f64 grace_period_end_time` (`+0x28`)<br>6. `f64 paid_until_time` (`+0x30`)<br>7. `u32 upkeep_status_flags` (`+0x38`)<br>8. `u8 is_foreclosed_or_locked` (`+0x3C`)<br>9. `u32 mithril_upkeep_cost` (`+0x40`) |
| `0x0801` (`2049`) | `HousingPurchaseEntry` | `0x140bb20f0` -> `0x140ab2080` / `0x140ab2110` | 1. `u64 house_id` (`+0x10`)<br>2. `u32 house_template_did` (`+0x18`)<br>3. `u32 coin_price` (`+0x1C`)<br>4. `u32 upkeep_cost` (`+0x20`)<br>5. `u32 mithril_or_writ_price` (`+0x24`)<br>6. `u32 house_category_flags` (`+0x28`) |
| `0x0803` (`2051`) | `HousingPurchaseList` | `0x140bb2190` -> `0x14032c580` / `0x14032cf00` | `Array<0x0801>` (`+0x08`: `2 × u32` array header + `N` serialized `0x0801` entries) |
| `0x0E9C` (`3740`) | `BankAddress` | `0x140bafcb0` -> `0x140a35730` / `0x140a35790` | 1. `u64 owner_or_house_iid` (`+0x10`)<br>2. `u32 bank_repository_type` (`+0x18`, `Enum_BankRepositoryType` `0x230002B8`)<br>3. `u32 chest_type` (`+0x1C`, `Enum_ChestType` `0x230001CD`) |
| `0x0C1F` (`3103`) | `BankChestInfo` | `0x140bafcf0` -> `0x140a35ca0` / `0x140a35d00` | 1. `u32 chest_type` (`+0x10`, `Enum_ChestType` `0x230001CD`)<br>2. `StringInfo chest_name` (`+0x18`, via `0x140343fb0`)<br>3. `u32 permission_or_capacity_flag` (`+0x58`) |
| `0x0A07` (`2567`) | `VaultItem` | `0x140bb5cd0` -> `0x140bc6b60` / `0x140bc6c00` | 1. `u64 item_iid` (`+0x10`)<br>2. `u64 container_or_owner_iid` (`+0x18`)<br>3. `PropertySet` (`+0x20`, class `0x00A6` via `0x14043a0f0`)<br>4. `Property` (`+0x80`, class `0x0027` via `0x140439770`, `UI_Examination_Tooltip_DID`)<br>5. `u8 is_bound_or_flag` (`+0xB0`) |

---

## 4. Clean-Room Specification: Mail, Auction House, and Housing Wire Protocol <a id="clean-room-spec"></a>

### 4.1 Purpose & Scope <a id="spec-purpose"></a>

This specification defines the clean-room wire protocol for the Mail System, Auction House, and Player & Kinship Housing (including Housing Chest and Escrow storage) over the WLib RPC framing layer. All multi-byte integers and IEEE-754 floating-point values are **little-endian**.

- **Provenance tags used in this section:**
  - `[OBS]`: Observed directly in the read-only game data archives (`0x30000000` WLib script archive and `0x34000000` Master Property Definition).
  - `[BIN]`: Confirmed from read-only static analysis of WState object serializers in the client executable.

### 4.2 WLib RPC Framing & Argument Encoding <a id="spec-framing"></a>

Every RPC invocation is carried inside a `NetBlob` payload using one of two WLib RPC envelopes `[OBS]`:

1. **Entity-Targeted RPC (`0x00D9:0x000D`):**
   - `u64 target_entity_iid`: 64-bit entity instance ID (`Mail` `0x0514`, `Auction` `0x04E5`, `PremiumHousing` `0x169F`, and `StorageLink` `0x0F5C`).
   - `u32 function_id`: `0x80000000 | (interface_class_id << 16) | method_index`.
   - `u32 arg_byte_length`: Byte length of the serialized argument payload.
   - `u8[arg_byte_length] arg_payload`: Arguments serialized sequentially with no per-argument type tags for primitive types.
2. **Broadcast / Service RPC (`0x00D9:0x000C`):**
   - `u32 function_id`: `0x80000000 | (interface_class_id << 16) | method_index` (`Housing` `0x07A7`, `BankStorage` `0x0A04`, and `ChestConfig` `0x0C20`).
   - `u32 arg_byte_length`: Byte length of the serialized argument payload.
   - `u8[arg_byte_length] arg_payload`: Arguments serialized sequentially.

When an argument is a **WState Object Reference** (`ref`), the wire stream encodes a `u32 class_id` (`0x00000000` for a null reference) followed immediately by the class-specific binary payload `[BIN]`.

---

### 4.3 Mail System Protocol (`0x0514`) <a id="spec-mail"></a>

#### 4.3.1 Mail RPCs (`0x0514`, Entity-Targeted) `[OBS]`

| `function_id` | Dir | Target Entity | Wire Arguments (Sequential Little-Endian Layout) | Semantics |
| --- | --- | --- | --- | --- |
| `0x85140074` | C→S | Mailbox IID | `u64 entity_iid` | Interact with world mailbox object (`Mail_ContainerID` `0x10000674`). |
| `0x85140048` | C→S | Avatar IID | `u32 is_cod_flag`, `ref<0x0068> attached_item_iids` (`Array<u64>`, max 10), `u32 cod_copper`, `u32 enclosed_copper`, `ref<0x00C7> body`, `ref<0x00C7> subject`, `ref<0x00C7> recipient_name`, `u64 mailbox_iid` | Send mail message. Subject and body are limited to `1024` characters each. |
| `0x85140040` | C→S | Avatar IID | `u64 mail_id`, `u64 mailbox_iid` | Open/read mail letter `mail_id`; requests full message body. |
| `0x8514003B` | C→S | Avatar IID | `u64 mail_id`, `u64 mailbox_iid` | Detach all attachments (items and coin; pays COD if `Mail_IsCOD`). |
| `0x8514006F` | C→S | Avatar IID | `u64 mail_id`, `u64 mailbox_iid` | Detach item attachment(s) only. |
| `0x85140092` | C→S | Avatar IID | `u64 mail_id`, `u64 mailbox_iid` | Detach currency attachment only. |
| `0x85140015` | C→S | Avatar IID | `u64 mail_id`, `u64 mailbox_iid` | Return mail letter `mail_id` to sender. |
| `0x85140012` | C→S | Avatar IID | `u64 mail_id`, `u64 mailbox_iid` | Delete mail letter `mail_id` (requires no remaining attachments). |
| `0x85140025` | S→C | Avatar IID | `u64 mailbox_iid` | Open Mailbox UI panel. |
| `0x85140035` | S→C | Avatar IID | *(0 bytes)* | Close Mailbox UI panel. |
| `0x85140088` | S→C | Avatar IID | `ref<0x00B6> inbox_table` (`HashTable<u64, MailLetter(0x01B5)>`) | Full inbox header table; sets `Mail_Count` (`0x100002B0`). |
| `0x8514007F` | S→C | Avatar IID | `ref<0x01B5> mail_letter` | Single `MailLetter` added or updated (includes body after `0x85140040`). |
| `0x85140026` | S→C | Avatar IID | `u64 detached_item_or_flag`, `u64 mail_id` | Attachment/currency detached from `mail_id`. |
| `0x85140036` | S→C | Avatar IID | `u64 mail_id` | Unlock `mail_id` after failed/cancelled operation. |
| `0x85140081` | S→C | Avatar IID | `u64 mail_id` | Remove `mail_id` from inbox; decrements `Mail_Count` (`0x100002B0`). |
| `0x85140083` | S→C | Avatar IID | *(0 bytes)* | Mail send succeeded. |
| `0x85140086` | S→C | Avatar IID | `u32 error_pid` | Mail operation failed with error `PropertyID`. |

#### 4.3.2 WState `0x01B5` (`MailLetter`) Wire Layout `[BIN, OBS]`

| Offset | Size | Type | Field | Description |
| --- | --- | --- | --- | --- |
| `+0x00` | `4` | `u32` | `flags` | Bitfield of boolean letter flags `[BIN]`:<br>- `0x00000001` (bit 0): `HasBeenOpened` / `IsRead` (`0x01B5:0x001C` get, `0x01B5:0x0019` set)<br>- `0x00000002` (bit 1): `IsReturned` (`0x01B5:0x001D` get, `0x01B5:0x001A` set)<br>- `0x00000004` (bit 2): `IsCOD` (`0x01B5:0x0021` get, `0x01B5:0x0020` set)<br>- `0x00000008` (bit 3): `IsAdmin` / `IsSystemMail` (`0x01B5:0x0024` get, `0x01B5:0x0025` set)<br>- `0x00000010` (bit 4): `IsSpam` (`0x01B5:0x0032` get, `0x01B5:0x0033` set)<br>- `0x00000020` (bit 5): `IsFaux` / `IsStoreMail` (`0x01B5:0x0034` get, `0x01B5:0x0035` set)<br>- `0x00000040` (bit 6): `CannotReturn` / `NoReturn` (`0x01B5:0x003A` get, `0x01B5:0x0037` set; `0x01B5:0x001F` `IsReturnable` evaluates `(flags & 0x40) == 0 && Mail_SenderIID != 0`) |
| `+0x04` | var | `PropertySet` (`0x00A6`) | `properties` | Keyed property map containing `0x100026F7` (`Mail_MailIID`, `u64`), `0x100003A6` (`Mail_ID`, `u64`), `0x10001233` (`Mail_SenderIID`, `u64`), `0x10000F90` (`Mail_SenderName`, `StringInfo`), `0x10000925` (`Mail_RecipientIID`, `u64`), `0x1000115A` (`Mail_RecipientName`, `StringInfo`), `0x10000926` (`Mail_Subject`, `StringInfo`), `0x10000E8A` (`Mail_Message`, `StringInfo`), `0x10001532` (`Mail_MailTimestamp`, `f64`), `0x1000057A` (`Mail_CurrencyAttachment`, `u32`), `0x10000673` (`Mail_CurrencyOnDelivery`, `u32`), `0x10000AD7` (`Mail_ItemIID`, `u64`), `0x100010EF` (`UI_Examination_Tooltip_DID`, `Struct`), and `0x10005E77` (`Mail_AdditionalItemAttachmentArray`, `Array` of `0x10005E76`) `[BIN, OBS]`. |

---

### 4.4 Auction House Protocol (`0x04E5`) <a id="spec-auction"></a>

#### 4.4.1 Auction House RPCs (`0x04E5`, Entity-Targeted on Avatar IID) `[OBS]`

| `function_id` | Dir | Wire Arguments (Sequential Little-Endian Layout) | Semantics |
| --- | --- | --- | --- |
| `0x84E50018` | C→S | `u64 auctioneer_iid`, `ref<0x0189> auction_adaptor` | Post a new auction (`Auction_Duration` `0x100003A3` in `{14400, 28800, 86400, 172800}` seconds). |
| `0x84E5001A` | C→S | `u64 auctioneer_iid`, `u32 bid_or_buyout_amount`, `ref<0x0189> auction_adaptor` | Place a bid or buyout (`bid_or_buyout_amount >= Auction_BuyoutPrice`). |
| `0x84E50015` | C→S | `u64 auctioneer_iid`, `u64 auction_id` | Cancel own posted auction `auction_id`. |
| `0x84E50017` | C→S | `u64 auctioneer_iid`, `u64 item_iid`, `u64 auction_id` | Request item examination tooltip (`0x10000421`) for `auction_id`. |
| `0x84E50026` | C→S | `u32 ah_currency_type` (`1`=Coin, `2`=Mithril), `u64 auctioneer_iid`, `u32 page_number`, `u32 sort_descending_flag`, `ref<0x0188> auction_query` | Execute Auction House search query. |
| `0x84E50003` | C→S | `u32 ah_currency_type`, `u64 auctioneer_iid` | Request own posted auctions list. |
| `0x84E5001D` | C→S | `u32 ah_currency_type`, `u64 auctioneer_iid` | Request own active bids list. |
| `0x84E50035` | C→S | `u32 ah_currency_type`, `u64 auctioneer_iid` | Refresh own posted auctions when dirty flag (`0x10001720` / `0x1000508C`) is set. |
| `0x84E50036` | C→S | `u32 ah_currency_type`, `u64 auctioneer_iid` | Refresh own active bids when dirty flag (`0x10001721` / `0x1000508B`) is set. |
| `0x84E50049` | C→S | `u32 close_or_mode_flag` | Notify server of Auction House UI close / mode change. |
| `0x84E5000F` | S→C | `u32 ah_currency_type`, `u64 auctioneer_iid` | Open Auction House UI panel. |
| `0x84E5002C` | S→C | *(0 bytes)* | Close Auction House UI panel. |
| `0x84E5001E` | S→C | `u32 ah_currency_type`, `u32 current_page`, `u32 total_pages_or_count`, `ref<0x00B6> results_table` (`HashTable<u64, 0x0189>`) | Search query results page. |
| `0x84E5000B` | S→C | `u32 ah_currency_type`, `ref<0x00B6> posted_table` (`HashTable<u64, 0x0189>`) | Own posted auctions list. |
| `0x84E50014` | S→C | `u32 ah_currency_type`, `ref<0x00B6> bids_table` (`HashTable<u64, 0x0189>`) | Own active bids list. |
| `0x84E50021` | S→C | `u32 status_or_error_pid` | Auction status or error notification `PropertyID`. |
| `0x84E50024` | S→C | `ref<0x0189> auction_adaptor` | Single auction listing added or updated. |
| `0x84E50027` | S→C | `ref<0x0189> auction_adaptor` | Single auction listing completed or removed. |
| `0x84E50043` | S→C | `u64 auction_id` | Auction listing removed by ID. |

#### 4.4.2 WState `0x0188` (`AuctionQuery`) & `0x0189` (`AuctionAdaptor`) Wire Layouts `[BIN, OBS]`

**WState `0x0188` (`AuctionQuery`):**

| Offset | Size | Type | Field | Description |
| --- | --- | --- | --- | --- |
| `+0x00` | `4` | `u32` | `sort_column_pid` | Sort property ID (`0x100026A5`, `0x100026CD`, `0x100026CE`, `0x100041CD`, `0x100041CB`, `0x100003A2`, `0x10001CB2`) `[BIN, OBS]`. |
| `+0x04` | `4` | `u32` | `array_header_flags` | Standard WState array capacity/flags word `[BIN]`. |
| `+0x08` | `4` | `u32` | `filter_count` | Number of filter `Property` elements (`<= 0x3FFFFFFF`) `[BIN]`. |
| `+0x0C` | var | `Property[filter_count]` | `filters` | Sequential `Property` (`0x0027`) elements (`0x10001533` `Auction_CategoryType`, `0x10000D95` `Auction_LevelRange`, `0x10000F8B` `Auction_StringSearch`, `0x10001593` `Auction_KinshipSearch`, `0x10001BB6` `Auction_SearchableProperty_Set`, `0x10002684` `Auction_IASearch`) `[BIN, OBS]`. |

**WState `0x0189` (`AuctionAdaptor`):**

| Offset | Size | Type | Field | Description |
| --- | --- | --- | --- | --- |
| `+0x00` | `8` | `u64` | `item_iid` | 64-bit instance ID of the auctioned item `[BIN]`. |
| `+0x08` | var | `PropertySet` (`0x00A6`) | `auction_properties` | Contains `0x10000002` (`Auction_ID`, `u64`), `0x1000507C` (`Auction_AuctionType`, `u32`), `0x10000AD0` (`Auction_Poster`, `u64`), `0x10000A07` (`Auction_PosterName`, `StringInfo`), `0x10000BAA` (`Auction_KinshipID`, `u64`), `0x1000074C` (`Auction_KinshipName`, `StringInfo`), `0x10000AD1` (`Auction_InitialPrice`, `u32`), `0x10000E84` (`Auction_BuyoutPrice`, `u32`), `0x10000D96` (`Auction_CurrentBid`, `u32`), `0x10000F8C` (`Auction_CurrentBidder`, `u64`), `0x100002AC` (`Auction_AuctionBids`, `u32`), `0x100003A3` (`Auction_Duration`, `u32`), `0x100003A2` (`Auction_ItemLevel`, `u32`), `0x100041CC` (`Auction_ItemQuantity`, `u32`), `0x10000D94` (`Auction_TimeReceived`, `f64`) `[BIN, OBS]`. |
| `var` | var | `Property` (`0x0027`) | `tooltip_property` | Serialized `0x10000421` (`UI_Examination_Tooltip_DID`) property for item inspection `[BIN, OBS]`. |

---

### 4.5 Player & Kinship Housing Protocol (`0x07A7`, `0x169F`, `0x0A04`) <a id="spec-housing"></a>

#### 4.5.1 Packed 64-Bit `house_id` Invariant `[OBS]`

Every `u64 house_id` transmitted in `0x07A7` RPC arguments or WState payloads (`0x07A8`, `0x0801`) satisfies:
- `neighborhood_did = u32(house_id >> 32)` (`0x10001A49` `HouseKeeper_NeighborhoodDID`)
- `house_did = u32(house_id & 0xFFFFFFFF)` (`0x1000190A` `HouseKeeper_HouseDID`)

#### 4.5.2 Summary of Housing & Storage RPCs `[OBS]`

- **Purchase / Abandon / Directory (`0x07A7` Broadcast):**
  - Purchase house: `0x87A7000B` (`u32 alt_currency_flag, u32 house_did, u32 neighborhood_did, u32 template_did, u32 vendor_did, u64 vendor_iid`) -> S→C `0x87A7002A` / `0x87A700AF`.
  - Abandon house: `0x87A70022` (`u64 house_id`) -> S→C `0x87A700AF` (`u64 status_or_owner_iid, u64 house_id`).
  - Query own/kinship houses: `0x87A7001E` (`0 bytes`).
  - Browse neighborhoods/houses: `0x87A70008` (`u32 template_did`) -> `0x87A70012` (`ref<0x07B1>, u32 status`); `0x87A7003B` (`u32 neighborhood_did`) -> `0x87A700B9` (`ref<0x07A9>, u32 status`); `0x87A70040` (`u32 neighborhood_did`) -> `0x87A700C2` (`ref<0x0803>, u32 status`).
- **Upkeep / Maintenance (`0x07A7` Broadcast):**
  - Query upkeep: `0x87A70019` (`u32 house_did, u32 neighborhood_did, u64 housekeeper_iid`) -> S→C `0x87A70062` (`ref<0x07C6>, u32 house_did, u32 neighborhood_did`).
  - Pay upkeep (coin): `0x87A7002D` (`u32 upkeep_amount, u64 house_id`) -> S→C `0x87A70051` (`u32 status, u32 house_did, u32 neighborhood_did`).
  - Pay upkeep (Mithril): `0x87A701DE` (`u32 periods, u64 house_id`) -> S→C `0x87A701E4` (`u32 status`).
- **Permissions (`0x07A7` Broadcast):**
  - Query permissions: `0x87A70006` (`u32 house_did, u32 neighborhood_did, u64 housekeeper_iid`) -> S→C `0x87A70042` (`ref<0x07BC>, u32 house_did, u32 neighborhood_did`).
  - Update group permission bitmask: `0x87A70016` (`u32 bitmask, u32 group_index, u32 house_did, u64 housekeeper_iid`) -> S→C `0x87A70049`.
  - Add/modify/remove individual guest: `0x87A70071` (add by IID), `0x87A70077` (add by `StringInfo` name), `0x87A70116` (modify bitmask), `0x87A700A5` (remove by IID) -> S→C `0x87A7004E` (`u32 status, u64 target_char_iid`).
- **Decoration Hooks (`0x07A7` Broadcast):**
  - Toggle decoration mode: `0x87A701B1` (`u32 enable_flag, u64 housekeeper_iid`).
  - Place item in hook: `0x87A701B5` (`u32 hook_id, u64 item_iid, u64 housekeeper_iid`) -> S→C `0x87A701AE` (`u32 hook_id, u64 item_iid, u32 status`).
  - Remove item from hook: `0x87A701B7` (`u32 hook_id, u64 housekeeper_iid`) -> S→C `0x87A701AE`.
  - Rotate item in hook: `0x87A701BF` (`f32 heading_degrees, u32 hook_id, u64 housekeeper_iid`) -> S→C `0x87A701BB` (`u32 hook_id, f32 heading_degrees, u32 status`).
  - Translate/offset item in hook: `0x87A701F4` (`ref<0x00D3> offset_vector, u64 housekeeper_iid, u32 hook_id`).
- **Housing Chest & Escrow Storage (`0x0A04` Broadcast):**
  - Open chest/escrow: `0x8A040004` (`u64 chest_or_npc_iid, ref<0x0E9C> bank_address`) -> S→C `0x8A04000C` (`ref<0x0E9C>, ref<Array<0x0A07>>, ref<Array<0x0C1F>>, u32 max_capacity, u32 used_capacity, u64 chest_iid`).
  - Deposit item: `0x8A040014` (`u32 quantity, u32 dest_chest_type, u64 item_iid, ref<0x0E9C> bank_address`) -> S→C `0x8A04001B`.
  - Withdraw item (chest or escrow): `0x8A040017` (`u32 quantity, u64 dest_container_iid, u64 item_iid, ref<0x0E9C> bank_address`) -> S→C `0x8A040020`.
  - Move item between chest tabs: `0x8A040018` (`u32 dest_chest_type, u64 item_iid, ref<0x0E9C> bank_address`).
  - Close chest/escrow: `0x8A040007` (`ref<0x0E9C> bank_address`).

---

### 4.6 Synthetic Test Vectors <a id="spec-test-vectors"></a>

#### 4.6.1 Mail Open Letter Request (`0x85140040`, C→S)
- **Scenario:** Player Avatar `0x00000001000000AA` opens Mail Letter `0x00000002000000BB` at Mailbox `0x00000003000000CC`.
- **Wire Bytes (Entity-Targeted `0x00D9:0x000D` payload):**
  - `target_entity_iid` (`u64` LE): `AA 00 00 00 01 00 00 00`
  - `function_id` (`u32` LE): `40 00 14 85` (`0x85140040`)
  - `arg_byte_length` (`u32` LE): `10 00 00 00` (`16` bytes)
  - `mail_id` (`u64` LE): `BB 00 00 00 02 00 00 00`
  - `mailbox_iid` (`u64` LE): `CC 00 00 00 03 00 00 00`

#### 4.6.2 Housing Decoration Rotate Request (`0x87A701BF`, C→S)
- **Scenario:** Player rotates decoration in Hook ID `12` (`0x0000000C`) to `90.0` degrees (`0x42B40000`) on HouseKeeper `0x00000004000000DD`.
- **Wire Bytes (Broadcast `0x00D9:0x000C` payload):**
  - `function_id` (`u32` LE): `BF 01 A7 87` (`0x87A701BF`)
  - `arg_byte_length` (`u32` LE): `10 00 00 00` (`16` bytes)
  - `heading_degrees` (`f32` LE): `00 00 B4 42` (`90.0f`)
  - `hook_id` (`u32` LE): `0C 00 00 00` (`12`)
  - `housekeeper_iid` (`u64` LE): `DD 00 00 00 04 00 00 00`

---

## Change Log <a id="change-log"></a>

| Date | Author | Summary |
| --- | --- | --- |
| 2026-10-20 | Spec writer (dirty room) | Initial reverse-engineering note and clean-room protocol specification for Mail (`0x0514`), Auction House (`0x04E5`), and Player & Kinship Housing (`0x07A7`, `0x169F`, `0x0A04`, `0x0C20`, `0x0F5C`). |
