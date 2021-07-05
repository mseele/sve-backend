package de.sve.backend.sheets;

import java.io.IOException;
import java.io.InputStream;
import java.security.GeneralSecurityException;
import java.util.Collections;
import java.util.List;

import com.google.api.client.googleapis.javanet.GoogleNetHttpTransport;
import com.google.api.client.json.gson.GsonFactory;
import com.google.api.services.sheets.v4.Sheets;
import com.google.api.services.sheets.v4.SheetsScopes;
import com.google.api.services.sheets.v4.model.Sheet;
import com.google.api.services.sheets.v4.model.SheetProperties;
import com.google.api.services.sheets.v4.model.Spreadsheet;
import com.google.api.services.sheets.v4.model.ValueRange;
import com.google.auth.http.HttpCredentialsAdapter;
import com.google.auth.oauth2.GoogleCredentials;

public abstract class AbstractSheetController {

	private static final String CREDENTIALS_FILE_PATH = "/credentials.json"; //$NON-NLS-1$

	private static final List<String> SCOPES = Collections.singletonList(SheetsScopes.SPREADSHEETS);

	protected Sheets fSheets;

	protected String fSpreadsheetId;

	public AbstractSheetController(String spreadsheetId) throws GeneralSecurityException, IOException {
		this.fSheets = new Sheets.Builder(GoogleNetHttpTransport.newTrustedTransport(), GsonFactory.getDefaultInstance(), credentials())
								 .setApplicationName("sve-backend-sheet-controller") //$NON-NLS-1$
								 .build();
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
					if (row == null || row.isEmpty() || row.get(0) == null) {
						return index;
					}
					index++;
				}
				if (values.size() < batch) {
					return index;
				}
			} else {
				return i;
			}
		}
		return -1; // never should be here
	}

	private static HttpCredentialsAdapter credentials() throws IOException {
		try (InputStream inputStream = AbstractSheetController.class.getResourceAsStream(CREDENTIALS_FILE_PATH)) {
			return new HttpCredentialsAdapter(GoogleCredentials.fromStream(inputStream)
					 										   .createScoped(SCOPES));
		}
	}

}
