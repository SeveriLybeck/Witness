# Gateway HTTP API

JSON over HTTP on the gateway's port (`contracts/ports.md`). Bodies are defined in `gateway.schema.json#/$defs/<def>`;
records are served as stored and validate against `contracts/schemas/`. `<key>` is `<oldCommit>-<newCommit>`.
Errors answer 4xx with `error` and, for a refused approval, the gate that refused it.

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/api/bundles` | | `bundles` |
| GET | `/api/bundles/<key>/<record>` | | the record file, e.g. `differences.json`; 404 when absent |
| GET | `/api/file?commit=<sha>&path=<file>` | | `fileText` |
| GET | `/api/bundles/<key>/viewers` | | `viewers`: plugins whose detect rules match the repository; `tabs` true when more than one |
| GET | `/api/bundles/<key>/pages/<plugin>/<path>` | | a file under that plugin's working folder for the bundle (read-only, type by extension; 404 outside it); a `page` result's relative `url` resolves here |
| POST | `/api/bundles/<key>/playground` | `playground` | `plugin-reply.schema.json` |
| GET | `/api/sessions/<key>/<reviewer>` | | `session.schema.json` |
| POST | `/api/sessions/<key>/<reviewer>` | `sessionUpdate` | `session.schema.json` |
| POST | `/api/sessions/<key>/<reviewer>/review` | `submit` | `submitted`, or `error` with `gate` |

- The reviewer's GitHub token travels in the `Authorization` header of the review request only; it is never stored.
- Before submitting, the gateway compares the pull request's head with `newCommit`; if it moved the answer is
  `error` with gate `head-moved`, and a new bundle is prepared in its own directory.
- `sessionUpdate` appends to `opened`, `shown` and `runs`, and sets one answer or the spot-check.
