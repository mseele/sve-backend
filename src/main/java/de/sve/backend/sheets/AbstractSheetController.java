package de.sve.backend.sheets;

import java.util.Collections;
import java.util.List;

import com.google.api.client.auth.oauth2.BearerToken;
import com.google.api.client.auth.oauth2.Credential;
import com.google.api.services.sheets.v4.SheetsScopes;
import com.google.appengine.api.appidentity.AppIdentityService;
import com.google.appengine.api.appidentity.AppIdentityServiceFactory;

public abstract class AbstractSheetController {

	private static final List<String> SCOPES = Collections.singletonList(SheetsScopes.SPREADSHEETS);

	protected static Credential authorize() {
		AppIdentityService appIdentity = AppIdentityServiceFactory.getAppIdentityService();
		AppIdentityService.GetAccessTokenResult accessToken = appIdentity.getAccessToken(SCOPES);
		Credential creds = new Credential(BearerToken.authorizationHeaderAccessMethod());
		creds.setAccessToken(accessToken.getAccessToken());
		return creds;
	}

}
