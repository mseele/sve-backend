package de.sve.backend.sheets;

import java.io.IOException;
import java.security.GeneralSecurityException;
import java.util.Collections;
import java.util.List;

import com.google.api.client.auth.oauth2.BearerToken;
import com.google.api.client.auth.oauth2.Credential;
import com.google.api.client.googleapis.javanet.GoogleNetHttpTransport;
import com.google.api.client.json.jackson2.JacksonFactory;
import com.google.api.services.sheets.v4.Sheets;
import com.google.api.services.sheets.v4.SheetsScopes;
import com.google.api.services.sheets.v4.model.Sheet;
import com.google.api.services.sheets.v4.model.SheetProperties;
import com.google.api.services.sheets.v4.model.Spreadsheet;
import com.google.api.services.sheets.v4.model.ValueRange;
import com.google.appengine.api.appidentity.AppIdentityService;
import com.google.appengine.api.appidentity.AppIdentityServiceFactory;

public abstract class AbstractSheetController {

	private static final List<String> SCOPES = Collections.singletonList(SheetsScopes.SPREADSHEETS);

	protected Sheets fSheets;
	
	protected String fSpreadsheetId;

	public AbstractSheetController(String spreadsheetId) throws GeneralSecurityException, IOException {
		Credential credential = authorize();
		this.fSheets = new Sheets.Builder(GoogleNetHttpTransport.newTrustedTransport(), JacksonFactory.getDefaultInstance(), credential).build();
		this.fSpreadsheetId = spreadsheetId;
	}

	protected String getSheetTitle(Integer sheetId) throws IOException {
		Spreadsheet spreadsheet = this.fSheets.spreadsheets().get(this.fSpreadsheetId).setFields("sheets(properties(sheetId,title))").execute(); //$NON-NLS-1$
		for (Sheet sheet : spreadsheet.getSheets()) {
			SheetProperties properties = sheet.getProperties();
			if (sheetId.equals(properties.getSheetId())) {
				return properties.getTitle();
			}
		}
		throw new IOException("Sheet with sheetId '" + sheetId + "' does not exist in spreadsheet '" + this.fSpreadsheetId + "'."); //$NON-NLS-1$ //$NON-NLS-2$ //$NON-NLS-3$
	}

	protected int getFirstEmptyRow(String sheetTitle, String column, int startRow) throws IOException {
		int maxRow = 1000;
		int batch = 100;
		for (int i = startRow; i <= maxRow; i+=batch) {
			String range = "'" + sheetTitle + "'!" + column + i+ ":" + column + (i+batch); //$NON-NLS-1$ //$NON-NLS-2$ //$NON-NLS-3$
			ValueRange response = this.fSheets.spreadsheets().values().get(this.fSpreadsheetId, range).execute();
			List<List<Object>> values = response.getValues();
			if (values != null) {
				int index = i;
				for (List<Object> row : values) {
					if (row.get(0) == null) {
						return index;
					}
					index++;
				}
			} else {
				return i;
			}
		}
		return -1; // never should be here
	}

	private static Credential authorize() {
		AppIdentityService appIdentity = AppIdentityServiceFactory.getAppIdentityService();
		AppIdentityService.GetAccessTokenResult accessToken = appIdentity.getAccessToken(SCOPES);
		Credential creds = new Credential(BearerToken.authorizationHeaderAccessMethod());
		creds.setAccessToken(accessToken.getAccessToken());
		return creds;
	}

}
