package de.sve.backend.sheets;

import java.io.IOException;
import java.util.Collections;
import java.util.List;

import com.google.api.client.auth.oauth2.BearerToken;
import com.google.api.client.auth.oauth2.Credential;
import com.google.api.services.sheets.v4.Sheets;
import com.google.api.services.sheets.v4.SheetsScopes;
import com.google.api.services.sheets.v4.model.Sheet;
import com.google.api.services.sheets.v4.model.SheetProperties;
import com.google.api.services.sheets.v4.model.Spreadsheet;
import com.google.appengine.api.appidentity.AppIdentityService;
import com.google.appengine.api.appidentity.AppIdentityServiceFactory;

public abstract class AbstractSheetController {

	private static final List<String> SCOPES = Collections.singletonList(SheetsScopes.SPREADSHEETS);

	protected static String getSheetTitle(Sheets sheets, String spreadsheetId, Integer sheetId) throws IOException {
		Spreadsheet spreadsheet = sheets.spreadsheets().get(spreadsheetId).setFields("sheets(properties(sheetId,title))").execute(); //$NON-NLS-1$
		for (Sheet sheet : spreadsheet.getSheets()) {
			SheetProperties properties = sheet.getProperties();
			if (sheetId.equals(properties.getSheetId())) {
				return properties.getTitle();
			}
		}
		throw new IOException("Sheet with sheetId '" + sheetId + "' does not exist in spreadsheet '" + spreadsheetId + "'."); //$NON-NLS-1$ //$NON-NLS-2$ //$NON-NLS-3$
	}

	protected static Credential authorize() {
		AppIdentityService appIdentity = AppIdentityServiceFactory.getAppIdentityService();
		AppIdentityService.GetAccessTokenResult accessToken = appIdentity.getAccessToken(SCOPES);
		Credential creds = new Credential(BearerToken.authorizationHeaderAccessMethod());
		creds.setAccessToken(accessToken.getAccessToken());
		return creds;
	}

}
