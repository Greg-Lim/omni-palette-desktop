
---
tags:
  - Implemented
---


for the application config file, we have the following attributes and relations

Entities:
- OS
- Application ID // Unique ID to prevent application conflict
- Application Name
- Application OS Name
- Application focus_state // Indicates if it needs to be in focus to activate
- Action ID
- Action Name
- Action CMD

Entities to add:
- Alias // To help when user is trying to select a command but typed wrongly
- Description // To figure if this is the right command to use 

application ID -> application name // to display
application ID + OS -> application_process_name // use to match the os is focused or background
action ID -> action name
action ID -> action cmd

action ID + focus_state -> is active