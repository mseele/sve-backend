# SVE Web Backend on Google App Engine Standard with Java 11

SVE Website Backend

## Deploying

First, check app.deploy.version in pom.xml is correct and everything works. Then:

```bash
 mvn clean package appengine:deploy
```

To view your app, use command:

```
gcloud app browse
```

Or navigate to `https://sve-backend.appspot.com`.
