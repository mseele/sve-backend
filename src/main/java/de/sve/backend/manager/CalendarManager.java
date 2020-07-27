package de.sve.backend.manager;

import java.io.IOException;
import java.security.GeneralSecurityException;
import java.util.List;

import de.sve.backend.calendar.CalendarService;
import de.sve.backend.model.calendar.Appointment;

public class CalendarManager {

	private static final String GENERAL_ID = "info@sv-eutingen.de"; //$NON-NLS-1$

	public static List<Appointment> appointments() throws GeneralSecurityException, IOException {
		return CalendarService.appointments(GENERAL_ID, 100);
	}

}
