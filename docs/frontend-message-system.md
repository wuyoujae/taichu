# Frontend Global Message System

## Overview

The frontend global message system provides a single application-wide API for user feedback and blocking confirmation flows. It is based on the visual design from `propertypes/components.html` and is implemented as a React-rendered global service.

The system is mounted once in `App.tsx` through `GlobalMessageProvider`. Business code can import the `message` API from `src/components/message` and call it directly from any page, component, or frontend utility module.

## Implemented Capabilities

- Toast notices: success, info, warning, and error.
- Promise-based confirm dialogs.
- Promise-based prompt dialogs.
- Manual toast close button.
- Auto-dismiss duration for toast notices.
- Responsive toast width on small screens.
- Global API request failure feedback through `apiClient`.

## File Structure

```text
app/frontend/taichu/src/components/message/
├── GlobalMessage.css
├── GlobalMessageProvider.tsx
├── index.ts
└── message.ts
```

## Usage

Import the global API:

```ts
import { message } from "../components/message";
```

Show a toast:

```ts
message.success("Agent saved", "Your changes have been successfully deployed.");
message.info("Update available", "A new model version is available.");
message.warn("API limit reaching", "You have used 90% of your monthly credits.");
message.error("Deployment failed", "Invalid API key provided.");
```

Use a confirm dialog:

```ts
const confirmed = await message.confirm({
  title: "Restart Agent?",
  desc: "This will clear the current conversation memory.",
  confirmText: "Restart",
});

if (confirmed) {
  message.success("Agent restarted");
}
```

Use a prompt dialog:

```ts
const folderName = await message.prompt({
  title: "New Folder",
  desc: "Enter a name for your new agent group.",
  placeholder: "Marketing Team",
  confirmText: "Create",
});

if (folderName) {
  message.success("Folder created", `Folder "${folderName}" has been created.`);
}
```

## API Request Integration

`apiClient` now routes failed HTTP responses, failed business responses, and network request failures through the global message system before rethrowing the error. This keeps request error handling consistent with the application UI standard.

## Design Notes

- The provider only renders UI. The `message` module owns the global state and exposes the direct-call API.
- Toasts are non-blocking and use the prototype's right-top placement and slide animation.
- Confirm and prompt dialogs return promises so business flows can remain linear and readable.
- The component uses existing design tokens when available and includes fallback values to avoid visual breakage on pages that do not define all CSS variables.
