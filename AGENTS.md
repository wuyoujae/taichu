# Behavioral Guidelines

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:

- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:

- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:

- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:

```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

# Coding Guidelines

## Code Style

1. Code must be written with a modular, reusable mindset and style. During implementation and file structure design, you should first evaluate whether the current module or feature is likely to require future extension or additional integrations, and then decide whether to reserve extension points accordingly. If not, do not introduce unnecessary abstractions or extra functionality. If you do not know or are unsure, ask first.

2. Code readability must be high. Do not produce messy, unmaintainable code; the codebase should remain clear, clean, and well-structured.

* Maintain proper indentation.
* Minimize the use of long comment blocks in code. Keep comments brief when needed, or omit them entirely. Comments should default to English.
* Naming in code must be concrete and meaningful. File names may use kebab-case, while function names and variables must use camelCase.

### frontend

1. After completing a new page or component, you must wire up the corresponding route or integrate the component where appropriate so the user can actually see the changes. You must also provide the correct access path and usage method so the user can navigate to and view the updated page or content.
2. All request configurations must use request interceptors, integrated together with our custom message component.
3. When designing any new component or page, you must reference the existing pages and components in the project to ensure that the visual language and design remain consistent across the entire application.
4. When building components or implementing interactions, you should appropriately add CSS animations or motion effects to make the interface feel polished and refined rather than stiff or flat. For example, when opening a dropdown, it should appear with a subtle easing transition rather than popping in abruptly. Likewise, for content that appears during page switches or scrolling, use tasteful entrance effects such as slide-up, fade-in, and similar motion patterns. This applies to both enter and exit transitions.
5. If required, page development must include proper multi-device responsiveness, covering mobile, desktop, and tablet layouts. If responsiveness is not required, you do not need to handle it. If this is unclear or unspecified, ask before proceeding.

### backend

1. When designing the database, never use foreign key constraints. Logical relationships are allowed—for example, a `user` table and a `wallet` table can be associated through `user_id`—but do not enforce them with foreign key constraints. If a relational database is used, this rule must be followed. If no relational database is involved, ignore this requirement.
2. All ID fields must use UUIDs, and each ID should be generated immediately before the SQL statement is created and executed, unless a special business rule requires otherwise.
3. Use numeric values for enums and status fields whenever possible, such as `0`, `1`, `2`. For example, for a common account status, `0` can represent normal/active, and `1` can represent disabled/unavailable. Be mindful of the distinction: for binary state enums such as normal vs abnormal, it is acceptable to use `0` and another value. But for multiple peer-level enum categories, such as membership types (`free`, `pro`, `ultra`), numbering should start from `1`, where `1` represents `free`, `2` represents `pro`, and `3` represents `ultra`. This should be treated as a standard convention.
4. When developing backend APIs, you must treat stability, completeness, and production readiness as the highest priorities, rather than merely satisfying basic functionality. Before writing any code for an API, you must first systematically analyze all relevant edge cases, invalid inputs, exceptional states, concurrency risks, permission issues, data consistency concerns, and special branches in the business flow, ensuring the API design does not only cover the “happy path” but instead prioritizes the complex scenarios that arise in real production environments. For every API, both database design and application logic must be considered together during the design process.
5. Use a RESTful API response format standard, and ensure the frontend interceptor is properly implemented to handle it accordingly.

### Database

When designing the database, you must always start from the backend logic that is actually being implemented at the current stage. First determine the minimum necessary data required to support the feature, and then design the schema strictly around those real requirements. Every field, every table, and every relationship must directly map to already-defined business logic and API behavior. Anything that cannot be directly justified by the current functional scope should not be added in advance.

For example, if the feature currently being developed is login, then the database should only include data directly related to login, such as user ID, username, password, verification code, basic identity markers, created time, updated time, and other essential fields. Beyond that, do not preemptively design membership tiers, points systems, subscription states, permission extensions, user profiling, or any other content that has not yet entered the actual development scope. When membership-related features are truly being developed later, then extend incrementally based on the existing user table—for example by adding related fields, creating new tables, or establishing new business relationships—instead of stuffing all possible future needs into the schema from the beginning.

You must always adhere to this principle: the database is not a speculative architectural blueprint, but a precise mapping of the current business logic. Database design should remain minimal, clear, implementable, and evolvable, so that it accurately supports the current feature set while still being able to expand naturally in future iterations. Your goal is not to design a database that merely looks comprehensive, but to design one that is strictly aligned with the current backend logic, professionally structured, production-stable, and free from unnecessary overengineering.

### Test

### Selection

1. When writing frontend code, regardless of what is being implemented, you must first check whether i18n is configured, and then make sure the corresponding language entries are added properly. Do not overlook i18n configuration.If it does not exist, ignore it. If you are unsure, ask the user first.
2. When completing a feature that requires configuration, place all configurable values in the environment file and read them from `env`. You must also maintain a `.env.example` file so users can quickly migrate and configure the project for production deployment.
3. The `.gitignore` file must be kept up to date. Configuration files, test code, and any other files that should not be committed must be added accordingly. After completing each task, you should check whether `.gitignore` needs to be updated.
4. After completing each task, you should create a Git commit, but only include the files you personally modified, since multiple agents may be working in parallel and you must not interfere with other agents’ work. The Git commit message must also be detailed and complete.

## Output style

After completing each task, you must tell me how to run the updated project and how to perform my own testing and acceptance checks. Even though I know you will test the changes yourself after implementation, I will also verify them independently, so you need to clearly explain how I should test and validate the modified project on my side.

## README

The `README` is the official project documentation that will ultimately be submitted to the Git repository. It must be written in a clear, professional, readable, and well-structured manner. The writing style must follow common GitHub Markdown conventions to ensure a good reading experience and clear content hierarchy on GitHub pages. A small amount of HTML is allowed where appropriate to enhance layout, such as centered titles, badges, line breaks, callout blocks, or image alignment, but readability must never be sacrificed for visual flair. The primary goal of the README is always to communicate project information clearly, not to overdecorate it.

The README must be written from the perspective of a first-time visitor to an open-source repository, so that anyone landing on the repo for the first time can quickly understand what the project is, why it exists, what problem it solves, how to install it, how to run it, what stage it is currently in, how the project is structured, and how future development should continue. The language should be direct, accurate, and natural. Avoid empty slogan-style wording, vague descriptions, or internal shorthand that only makes sense to the original developers. The content must be complete, but not bloated. Every section should serve the goal of helping others quickly understand and get started with the project.

At a minimum, the README must include the following sections in sufficient detail, and should proactively include any other necessary sections as needed so that it becomes a complete project guide:

1. Project Overview: Explain the project name, positioning, core goals, primary use cases, and the key problem it is intended to solve.
2. Project Approach: Describe the overall design philosophy, product thinking, or system architecture rationale behind the project, explaining why it is designed this way rather than only listing surface-level features.
3. Core Features: List the major functional modules that have already been implemented or are planned, preferably in a modular format.
4. Tech Stack: Clearly list the frontend, backend, database, testing frameworks, build tools, and other key dependencies.
5. Project Structure: Explain the repository layout so readers can quickly understand the responsibility of each directory.
6. Quick Start: Provide complete steps for environment setup, dependency installation, configuration, and local startup, so others can follow along and run the project directly.
7. Configuration: Document environment variables, config files, database initialization, port settings, and other runtime requirements.
8. Development Guide: Describe the local development workflow, development conventions, code organization, API development standards, and any other important engineering rules.
9. Testing Guide: Explain how to run tests, what areas are covered, and what the current testing strategy is.
10. Usage Examples: Provide the most basic usage flow, runtime examples, or screenshots/demo instructions so readers can understand the project more intuitively.
11. Roadmap / Development Plan: Explain the current progress, what has already been completed, and what is planned next.
12. FAQ: Add common installation issues, runtime problems, and development caveats to reduce repeated communication costs.
13. Contribution Guide: If the project accepts collaboration, clearly document how to contribute, commit conventions, and basic collaboration requirements.
14. License: Clearly state the project’s open-source license or copyright terms.

When writing the README, you must not mechanically list headings without substance. Each section should be complete, specific, and credible based on the actual state of the project. The final README must reach this standard: even if a user has never heard of the project before, they should be able to gain a full understanding of it and have the basic ability to install, run, and continue developing it just by reading the README.

If the project is still in an early stage, that must be stated honestly. Do not pretend the project is already mature. The README should truthfully reflect the current state of the project while still maintaining a professional tone and strong structure. Your goal is not merely to “write a README file,” but to produce a high-quality README that genuinely serves as the project’s public face, documentation manual, and developer entry point.

## Required Thinking

Think in terms of the development standards used by large companies—if that is how enterprise teams do it, then that is how we should do it as well. You are not building a toy project; you are building an enterprise-grade commercial application, so you must fully account for the following factors:

1. Security: Data handling must follow the highest security standards. This includes secure storage, encryption and decryption, human verification, validation mechanisms, and secure interaction flows. In addition to data security, we must also consider privacy compliance and legal safety. When external libraries are needed, prefer open-source libraries that are commercially usable to avoid legal risk. If that is not feasible, we should implement the solution ourselves.

2. Performance: Performance must be carefully considered during both development and validation. Dangerous issues such as memory leaks must not be allowed. During implementation, always choose the most performant practical solution. After every development task, you should actively review whether any performance issues were introduced and whether further optimization is possible. Load speed is critical, so all reasonable performance optimization strategies should be considered. For example, if we are building an image upload feature and the user uploads a 3 MB image, we already know that such an image will load slowly in real-world usage. Therefore, the upload flow should include an intermediate processing step for image compression. Before storing the image in object storage, we should compress it as much as possible—for example, down to around 25 KB—using the best available compression strategy.

3. Stability: The system must not crash unpredictably. We need robust fallback strategies for all critical flows. For example, when calling AI models, if all model providers fail, we must already have a defined fallback mechanism in place. Every possible failure scenario should be carefully analyzed and backed by a fallback strategy. No design should be allowed that could damage the platform in any way.

4. Interaction: User interaction is extremely important. All interaction design should be approached from the user’s perspective and guided by sound product and UX principles, such as reducing user effort and encouraging low-friction behavior. These principles should shape not only frontend interaction details, but also backend flow design and overall system behavior.

If, before development begins, you do not know how leading companies in the market typically handle a given problem, you may first use search tools to study industry practices, record the findings, and then continue development.

# Docs

## Documentation Maintenance

Our documentation is organized using the **model** as the boundary. Each model may include database design, page design, backend design, frontend design, and other related parts. A model represents a composite collection of business functionality. Every document must be named based on the corresponding business feature, and the naming must be clear, explicit, and descriptive.

The content of the documentation may become outdated, so you should keep it updated in real time. If you discover that the documentation conflicts with the requirements of the feature currently being developed, you should ask the user to confirm the final approach before updating the document.

Each document should contain explanatory text, code, and relevant domain knowledge. Every document must also include key information such as code examples, usage examples, and solution guidance.

Do not put the contents of multiple unrelated modules into a single document. Documentation must be split in a modular way, while also avoiding the creation of an excessive number of documents.

**！！！！After completing each development task, you should review and determine whether the documentation needs to be updated.**If you determine that something should be documented but no existing documentation is available, do not hesitate to create a new document and record it clearly.

## Documentation Reading

We should adopt a **progressive disclosure** approach when loading documentation. During each development task, only read the documents that are relevant to the current feature, and always start with the most relevant ones. You may search by related keywords or directly read the document with the most relevant name.

Before developing any related feature or business module, you should first check the documentation to see whether there is already relevant knowledge or code information available, and then decide how to proceed with the implementation.

## Behavioral Guidelines

After completing each task, you must first check the following:

1. Check whether any content from the current development work needs to be recorded in the docs, and update the documentation promptly.
2. Check whether the corresponding document already exists. If it exists, modify or overwrite it directly.
3. If the document does not exist, create a new document to clearly record the development information from this task.

# PROJECT.md

Before you begin working, you must add `PROJECT.md` to your context. It is located in the `/docs` directory and provides a high-level overview of the entire project.

# Project-Specific Instructions

## Directory Configuration

There are three working subdirectories under the project root:

* `app`: All development-related code must be placed in this directory. It is divided into four subdirectories: `frontend`, `backend`, `selection`, and `test`, which are used to store frontend code, backend code, and other related code respectively. The `selection` directory is for temporarily placing code when it is not yet clear which location it should belong to; once confirmed, it should be moved to the proper directory. The `test` directory is used to store all testing-related code, including test scripts and similar files.
* `docs`: This is where all documentation for the project development is stored.
* `propertypes`: This directory contains the project’s prototype designs, and all frontend development must reference it.
* `resources`: All project assets and materials used during development must be placed in this directory.

## Technology Stack

Frontend: Electron-react
Backend: Rust ，Rust Axum
Databases: 使用文件持久化不使用db
i18n：en and zh-CN
License：MIT

# Requirements

After completing each task, you must say: “My task is completed! I’m so happy!!! Jae-ge, is there anything else you’d like me to finish? I will keep working hard and pushing forward!!!!”

# Response

All responses and explanations must be in Chinese. Code and comments may be in English.

# Must Require

Do **not** use the following commands:

* `del /s`
* `rd /s`
* `rmdir /s`
* `Remove-Item -Recurse`
* `rm -rf`

When deleting files, you must delete only **one explicitly specified file at a time**.

Correct example:

```powershell
Remove-Item "C:\path\to\file.txt"
```

If you need to delete multiple files, **halt the operation** and prompt the user to perform the deletions manually.


